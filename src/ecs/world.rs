use std::sync::{atomic::AtomicU32, Arc};

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use super::{
    resource::{Res, ResMut, Resource},
    system::{SystemGraph, SystemId},
    Bundle, Component, EcsError, Entity, System,
};

#[derive(Clone)]
pub struct ComponentPtr {
    pub component_id: u64,
    pub component: Arc<RwLock<dyn Component>>,
}

pub type Components = FxHashMap<Entity, FxHashMap<u64, ComponentPtr>>;

#[derive(Default)]
pub struct World {
    next_entity_id: AtomicU32,
    pub(crate) components: Arc<RwLock<Components>>,
    pub(crate) startup_systems: RwLock<SystemGraph>,
    pub(crate) systems: RwLock<SystemGraph>,
    pub(crate) resources: FxHashMap<u64, Arc<RwLock<dyn Resource>>>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_entity(&self) -> Entity {
        let id = self
            .next_entity_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let entity = Entity::new(id, 0);
        self.components.write().insert(entity, FxHashMap::default());
        entity
    }

    pub fn spawn<T: Bundle>(&self, bundle: T) -> anyhow::Result<Entity> {
        bundle.build(self)
    }

    pub fn add_component<T: Component>(&self, entity: Entity, component: T) -> anyhow::Result<()> {
        if self.has_component::<T>(entity) {
            return Err(EcsError::ComponentAlreadyExists.into());
        }
        let component = Arc::new(RwLock::new(component));
        self.components.write().entry(entity).or_default().insert(
            T::component_id(),
            ComponentPtr {
                component_id: T::component_id(),
                component,
            },
        );
        Ok(())
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        if let Some(components) = self.components.write().get_mut(&entity) {
            components.remove(&T::component_id());
        }
    }

    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        if let Some(components) = self.components.read().get(&entity) {
            components.contains_key(&T::component_id())
        } else {
            false
        }
    }

    pub fn remove_entity(&self, entity: Entity) {
        self.components.write().remove(&entity);
    }

    pub fn insert_resource<T: Resource>(&mut self, resource: T) -> anyhow::Result<()> {
        if self.has_resource::<T>() {
            return Err(EcsError::ResourceAlreadyExists.into());
        }
        let resource = Arc::new(RwLock::new(resource));
        self.resources.insert(T::resource_id(), resource);
        Ok(())
    }

    pub fn read_resource<T: Resource>(&self) -> anyhow::Result<Res<T>> {
        let resource = self
            .resources
            .get(&T::resource_id())
            .ok_or(EcsError::ResourceDoesNotExist)?;
        Ok(Res::new(resource.read()))
    }

    pub fn write_resource<T: Resource>(&self) -> anyhow::Result<ResMut<T>> {
        let resource = self
            .resources
            .get(&T::resource_id())
            .ok_or(EcsError::ResourceDoesNotExist)?;

        Ok(ResMut::new(resource.write()))
    }

    pub fn has_resource<T: Resource>(&self) -> bool {
        self.resources.contains_key(&T::resource_id())
    }

    pub fn has_startup_system(&self, system: SystemId) -> bool {
        self.startup_systems.read().has_system(system)
    }

    pub fn add_startup_system<S: System + 'static>(&self, system: S) -> SystemId {
        self.startup_systems.write().add_system(Arc::new(system))
    }

    pub fn add_startup_system_after<S: System + 'static>(
        &self,
        system: S,
        after: SystemId,
    ) -> SystemId {
        self.startup_systems
            .write()
            .add_system_after(Arc::new(system), after)
    }

    pub fn add_startup_system_before<S: System + 'static>(
        &self,
        system: S,
        before: SystemId,
    ) -> SystemId {
        self.startup_systems
            .write()
            .add_system_before(Arc::new(system), before)
    }

    pub fn add_startup_system_dependency(&self, dependency: SystemId, dependent: SystemId) {
        self.startup_systems
            .write()
            .add_dependency(dependency, dependent);
    }

    pub fn has_system(&self, system: SystemId) -> bool {
        self.systems.read().has_system(system)
    }

    pub fn add_system<S: System + 'static>(&self, system: S) -> SystemId {
        self.systems.write().add_system(Arc::new(system))
    }

    pub fn add_system_after<S: System + 'static>(&self, system: S, after: SystemId) -> SystemId {
        self.systems
            .write()
            .add_system_after(Arc::new(system), after)
    }

    pub fn add_system_before<S: System + 'static>(&self, system: S, before: SystemId) -> SystemId {
        self.systems
            .write()
            .add_system_before(Arc::new(system), before)
    }

    pub fn add_system_dependency(&self, dependency: SystemId, dependent: SystemId) {
        self.systems.write().add_dependency(dependency, dependent);
    }

    pub fn startup(world: &Arc<RwLock<Self>>) -> anyhow::Result<()> {
        world.read().startup_systems.read().run(&world.read())
    }

    pub fn update(world: &Arc<RwLock<Self>>) -> anyhow::Result<()> {
        world.read().systems.read().run_parallel(world)
    }
}
