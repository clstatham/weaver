use std::sync::Arc;

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use crate::{
    bundle::Bundle,
    entity::Entity,
    id::DynamicId,
    prelude::Component,
    query::{DynamicQueryBuilder, Query, QueryFilter, Queryable},
    resource::{Res, ResMut, Resource},
    storage::Components,
    system::System,
    system::{SystemGraph, SystemId, SystemStage},
};

pub struct World {
    pub(crate) components: Components,
    pub(crate) systems: FxHashMap<SystemStage, Arc<RwLock<SystemGraph>>>,
    pub(crate) resources: FxHashMap<DynamicId, Arc<RwLock<dyn Resource>>>,
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

    pub fn add_component<T: Component>(&mut self, entity: &Entity, component: T) {
        self.components.add_component(entity, component);
    }

    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> Entity {
        bundle.build(&mut self.components)
    }

    pub fn despawn(&mut self, entity: Entity) {
        self.components.despawn(entity.id());
    }

    pub fn add_resource<T: Resource>(&mut self, resource: T) -> anyhow::Result<()> {
        if self.has_resource::<T>() {
            return Err(anyhow::anyhow!("Resource already exists"));
        }
        let resource = Arc::new(RwLock::new(resource));
        let id = self.components.registry().get_static::<T>();
        self.resources.insert(id, resource);
        Ok(())
    }

    pub fn read_resource<T: Resource>(&self) -> anyhow::Result<Res<T>> {
        let id = self.components.registry().get_static::<T>();
        let resource = self
            .resources
            .get(&id)
            .ok_or(anyhow::anyhow!("Resource does not exist"))?;
        Ok(Res::new(resource.read()))
    }

    pub fn write_resource<T: Resource>(&self) -> anyhow::Result<ResMut<T>> {
        let id = self.components.registry().get_static::<T>();
        let resource = self
            .resources
            .get(&id)
            .ok_or(anyhow::anyhow!("Resource does not exist"))?;

        Ok(ResMut::new(resource.write()))
    }

    pub fn has_resource<T: Resource>(&self) -> bool {
        let id = self.components.registry().get_static::<T>();
        self.resources.contains_key(&id)
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
            systems
                .write()
                .autodetect_dependencies(world_lock.components.registry())?;
            drop(world_lock);
            systems.read().run(world)?;
        }
        Ok(())
    }

    pub fn run_stage_parallel(world: &Arc<RwLock<Self>>, stage: SystemStage) -> anyhow::Result<()> {
        let world_lock = world.read();
        if let Some(systems) = world_lock.systems.get(&stage).cloned() {
            systems
                .write()
                .autodetect_dependencies(world_lock.components.registry())?;
            drop(world_lock);
            systems.read().run_parallel(world)?;
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

    pub fn query_dynamic(&self) -> DynamicQueryBuilder<'_> {
        DynamicQueryBuilder::new(&self.components)
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
