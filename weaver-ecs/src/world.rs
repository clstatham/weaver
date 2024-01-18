use std::{any::TypeId, borrow::Cow, fmt::Debug, sync::Arc};

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use crate::{
    query::{Query, QueryFilter},
    Queryable,
};

use super::{
    resource::{Res, ResMut, Resource},
    storage::Components,
    system::{SystemGraph, SystemId, SystemStage},
    Bundle, Component, Entity, System,
};

#[derive(Clone)]
pub struct ComponentPtr {
    pub component_id: TypeId,
    pub component_name: Cow<'static, str>,
    pub component: Arc<RwLock<dyn Component>>,
}

impl Debug for ComponentPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentPtr")
            .field("component_id", &self.component_id)
            .field("component_name", &self.component_name)
            .finish()
    }
}

pub struct World {
    pub(crate) components: Components,
    pub(crate) systems: FxHashMap<SystemStage, Arc<RwLock<SystemGraph>>>,
    pub(crate) resources: FxHashMap<TypeId, Arc<RwLock<dyn Resource>>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            components: Components::default(),
            systems: FxHashMap::default(),
            resources: FxHashMap::default(),
        }
    }

    pub fn create_entity(&mut self) -> Entity {
        self.components.create_entity()
    }

    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> Entity {
        bundle.build(&mut self.components)
    }

    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) {
        let component = Arc::new(RwLock::new(component));
        self.components.add_component(
            entity.id(),
            ComponentPtr {
                component_id: TypeId::of::<T>(),
                component_name: Cow::Borrowed(std::any::type_name::<T>()),
                component,
            },
        );
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        self.components
            .remove_component(entity.id(), TypeId::of::<T>());
    }

    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        if let Some(components) = self.components.entity_components.get(&entity.id()) {
            components.contains_component(&TypeId::of::<T>())
        } else {
            false
        }
    }

    pub fn despawn(&mut self, entity: Entity) {
        self.components.despawn(entity.id());
    }

    pub fn insert_resource<T: Resource>(&mut self, resource: T) -> anyhow::Result<()> {
        if self.has_resource::<T>() {
            return Err(anyhow::anyhow!("Resource already exists"));
        }
        let resource = Arc::new(RwLock::new(resource));
        self.resources.insert(TypeId::of::<T>(), resource);
        Ok(())
    }

    pub fn read_resource<T: Resource>(&self) -> anyhow::Result<Res<T>> {
        let resource = self
            .resources
            .get(&TypeId::of::<T>())
            .ok_or(anyhow::anyhow!("Resource does not exist"))?;
        Ok(Res::new(resource.read()))
    }

    pub fn write_resource<T: Resource>(&self) -> anyhow::Result<ResMut<T>> {
        let resource = self
            .resources
            .get(&TypeId::of::<T>())
            .ok_or(anyhow::anyhow!("Resource does not exist"))?;

        Ok(ResMut::new(resource.write()))
    }

    pub fn has_resource<T: Resource>(&self) -> bool {
        self.resources.contains_key(&TypeId::of::<T>())
    }

    pub fn add_system<T: System + 'static>(&mut self, system: T) -> SystemId {
        self.add_system_to_stage(system, SystemStage::default())
    }

    pub fn add_system_to_stage<T: System + 'static>(
        &mut self,
        system: T,
        stage: SystemStage,
    ) -> SystemId {
        let system = Arc::new(system);

        self.systems
            .entry(stage)
            .or_default()
            .write()
            .add_system(system)
    }

    pub fn run_stage(world: &Arc<RwLock<Self>>, stage: SystemStage) -> anyhow::Result<()> {
        let world_lock = world.read();
        if let Some(systems) = world_lock.systems.get(&stage).cloned() {
            drop(world_lock);
            systems.write().autodetect_dependencies()?;
            systems.read().run(world.clone())?;
        }
        Ok(())
    }

    pub fn query<'a, T: Queryable<'a, ()>>(&'a self) -> Query<'a, T, ()> {
        Query::new(&self.components)
    }

    pub fn query_filtered<'a, T: Queryable<'a, F>, F: QueryFilter<'a>>(
        &'a self,
    ) -> Query<'a, T, F> {
        Query::new(&self.components)
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
