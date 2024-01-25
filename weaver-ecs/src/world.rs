use std::{path::Path, sync::Arc};

use atomic_refcell::{AtomicRef, AtomicRefMut};
use parking_lot::RwLock;
use petgraph::prelude::NodeIndex;
use rustc_hash::FxHashMap;

use crate::{
    bundle::Bundle,
    component::Data,
    entity::Entity,
    prelude::Component,
    query::{DynamicQueryBuilder, Query, QueryFilter, Queryable},
    registry::DynamicId,
    storage::{Components, SparseSet},
    system::{DynamicSystem, System},
    system::{SystemGraph, SystemStage},
};

pub struct World {
    pub(crate) components: Components,
    pub(crate) systems: FxHashMap<SystemStage, Arc<RwLock<SystemGraph>>>,
    pub(crate) resources: SparseSet<DynamicId, Data>,
}

impl World {
    pub fn new() -> Self {
        Self {
            components: Components::new(),
            systems: FxHashMap::default(),
            resources: SparseSet::default(),
        }
    }

    pub fn create_entity(&mut self) -> Entity {
        self.components.create_entity()
    }

    pub fn add_component<T: Component>(
        &mut self,
        entity: &Entity,
        component: T,
        field_name: Option<&str>,
    ) {
        self.components.add_component(entity, component, field_name);
    }

    pub fn add_dynamic_component(&mut self, entity: &Entity, component: crate::component::Data) {
        self.components.add_dynamic_component(entity, component);
    }

    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> Entity {
        bundle.build(&mut self.components)
    }

    pub fn despawn(&mut self, entity: Entity) {
        self.components.despawn(entity.id());
    }

    pub fn add_resource<T: Component>(&mut self, resource: T) -> anyhow::Result<()> {
        if self.has_resource::<T>() {
            return Err(anyhow::anyhow!("Resource already exists"));
        }
        let resource = resource.into_dynamic_data(None, self.registry());
        self.resources.insert(resource.type_id(), resource);
        Ok(())
    }

    pub fn add_dynamic_resource(&mut self, resource: Data) -> anyhow::Result<()> {
        if self.resources.contains(&resource.type_id()) {
            return Err(anyhow::anyhow!("Resource already exists"));
        }
        self.resources.insert(resource.type_id(), resource);
        Ok(())
    }

    pub fn read_resource<T: Component>(&self) -> anyhow::Result<AtomicRef<'_, T>> {
        let id = self.registry().get_static::<T>();
        let resource = self
            .resources
            .get(&id)
            .ok_or(anyhow::anyhow!("Resource does not exist"))?;
        Ok(resource.get_as())
    }

    pub fn write_resource<T: Component>(&self) -> anyhow::Result<AtomicRefMut<'_, T>> {
        let id = self.registry().get_static::<T>();
        let resource = self
            .resources
            .get(&id)
            .ok_or(anyhow::anyhow!("Resource does not exist"))?;

        Ok(resource.get_as_mut())
    }

    pub fn dynamic_resource(&self, id: DynamicId) -> anyhow::Result<&Data> {
        let resource = self
            .resources
            .get(&id)
            .ok_or(anyhow::anyhow!("Resource does not exist"))?;
        Ok(resource)
    }

    pub fn dynamic_resource_mut(&mut self, id: DynamicId) -> anyhow::Result<&mut Data> {
        let resource = self
            .resources
            .get_mut(&id)
            .ok_or(anyhow::anyhow!("Resource does not exist"))?;
        Ok(resource)
    }

    pub fn has_resource<T: Component>(&self) -> bool {
        let id = self.components.registry().get_static::<T>();
        self.resources.contains(&id)
    }

    pub fn add_system<T: System>(&mut self, system: T) -> NodeIndex {
        self.add_system_to_stage(system, SystemStage::default())
    }

    pub fn add_system_to_stage<T: System>(&mut self, system: T, stage: SystemStage) -> NodeIndex {
        self.systems
            .entry(stage)
            .or_default()
            .write()
            .add_system(system, self.components.registry())
    }

    pub fn add_dynamic_system_to_stage(
        &mut self,
        system: DynamicSystem,
        stage: SystemStage,
    ) -> NodeIndex {
        self.systems
            .entry(stage)
            .or_default()
            .write()
            .add_dynamic_system(system)
    }

    pub fn add_script(world: &Arc<RwLock<Self>>, script_path: impl AsRef<Path>) {
        DynamicSystem::load_script(script_path, world.clone()).unwrap();
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

    pub fn dynamic_id<T: Component>(&self) -> DynamicId {
        self.components.registry().get_static::<T>()
    }

    pub fn named_id(&self, name: &str) -> DynamicId {
        self.components.registry().get_named(name)
    }

    pub fn registry(&self) -> &Arc<crate::registry::Registry> {
        self.components.registry()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
