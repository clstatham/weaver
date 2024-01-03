use std::{
    cell::RefCell,
    sync::{atomic::AtomicU32, Arc, RwLock},
};

use rustc_hash::FxHashMap;

use super::{
    query::Queryable,
    resource::{Res, ResMut, Resource},
    system::{SystemGraph, SystemId},
    Bundle, Component, Entity, System,
};

pub type EntitiesAndComponents = FxHashMap<Entity, FxHashMap<u64, Arc<RwLock<dyn Component>>>>;

#[derive(Default)]
pub struct World {
    next_entity_id: AtomicU32,
    pub(crate) entities_components: EntitiesAndComponents,
    pub(crate) startup_systems: RefCell<SystemGraph>,
    pub(crate) systems: RefCell<SystemGraph>,
    pub(crate) resources: FxHashMap<u64, Arc<RwLock<dyn Resource>>>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_entity(&mut self) -> Entity {
        let id = self
            .next_entity_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let entity = Entity::new(id, 0);
        self.entities_components
            .insert(entity, FxHashMap::default());
        entity
    }

    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> Entity {
        bundle.build(self)
    }

    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) {
        let component = Arc::new(RwLock::new(component));
        self.entities_components
            .entry(entity)
            .or_default()
            .insert(T::component_id(), component);
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        if let Some(components) = self.entities_components.get_mut(&entity) {
            components.remove(&T::component_id());
        }
    }

    pub fn insert_resource<T: Resource>(&mut self, resource: T) {
        let resource = Arc::new(RwLock::new(resource));
        self.resources.insert(T::resource_id(), resource);
    }

    pub fn read_resource<T: Resource>(&self) -> Res<T> {
        let resource = self.resources.get(&T::resource_id()).unwrap();
        Res::new(resource.try_read().unwrap())
    }

    pub fn write_resource<T: Resource>(&self) -> ResMut<T> {
        let resource = self.resources.get(&T::resource_id()).unwrap();
        ResMut::new(resource.try_write().unwrap())
    }

    pub fn query<'w, 'q, 'i, Q>(&'w self) -> Q
    where
        'w: 'q,
        'q: 'i,
        Q: Queryable<'w, 'q, 'i>,
    {
        Q::create(&self.entities_components)
    }

    pub fn add_startup_system<S: System + 'static>(&self, system: S) -> SystemId {
        self.startup_systems
            .borrow_mut()
            .add_system(Box::new(system))
    }

    pub fn add_startup_system_after<S: System + 'static>(
        &self,
        system: S,
        after: SystemId,
    ) -> SystemId {
        self.startup_systems
            .borrow_mut()
            .add_system_after(Box::new(system), after)
    }

    pub fn add_startup_system_before<S: System + 'static>(
        &self,
        system: S,
        before: SystemId,
    ) -> SystemId {
        self.startup_systems
            .borrow_mut()
            .add_system_before(Box::new(system), before)
    }

    pub fn add_startup_system_dependency(&self, dependency: SystemId, dependent: SystemId) {
        self.startup_systems
            .borrow_mut()
            .add_dependency(dependency, dependent);
    }

    pub fn add_system<S: System + 'static>(&self, system: S) -> SystemId {
        self.systems.borrow_mut().add_system(Box::new(system))
    }

    pub fn add_system_after<S: System + 'static>(&self, system: S, after: SystemId) -> SystemId {
        self.systems
            .borrow_mut()
            .add_system_after(Box::new(system), after)
    }

    pub fn add_system_before<S: System + 'static>(&self, system: S, before: SystemId) -> SystemId {
        self.systems
            .borrow_mut()
            .add_system_before(Box::new(system), before)
    }

    pub fn add_system_dependency(&self, dependency: SystemId, dependent: SystemId) {
        self.systems
            .borrow_mut()
            .add_dependency(dependency, dependent);
    }

    pub fn startup(&self) {
        self.startup_systems.borrow().run(self);
    }

    pub fn update(&self) {
        self.systems.borrow().run(self);
    }
}
