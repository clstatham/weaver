use std::{path::Path, sync::Arc};

use parking_lot::{MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock};
use petgraph::prelude::NodeIndex;
use rustc_hash::FxHashMap;

use crate::{
    bundle::Bundle,
    component::Data,
    entity::Entity,
    prelude::Component,
    query::{DynamicQueryBuilder, Query, QueryFilter, Queryable},
    registry::DynamicId,
    script::Script,
    storage::{Components, SparseSet},
    system::{DynamicSystem, System},
    system::{SystemGraph, SystemStage},
};

pub struct World {
    pub components: Components,
    pub systems: RwLock<FxHashMap<SystemStage, Arc<RwLock<SystemGraph>>>>,
    pub resources: SparseSet<DynamicId, Data>,
    pub script_systems: Arc<RwLock<FxHashMap<String, (Script, Vec<(SystemStage, NodeIndex)>)>>>,
}

impl Component for Arc<RwLock<World>> {
    fn type_name() -> &'static str {
        "World"
    }
}

impl World {
    pub fn new() -> Self {
        Self {
            components: Components::new(),
            systems: RwLock::new(FxHashMap::from_iter(vec![
                (
                    SystemStage::Startup,
                    Arc::new(RwLock::new(SystemGraph::default())),
                ),
                (
                    SystemStage::PreUpdate,
                    Arc::new(RwLock::new(SystemGraph::default())),
                ),
                (
                    SystemStage::Update,
                    Arc::new(RwLock::new(SystemGraph::default())),
                ),
                (
                    SystemStage::PostUpdate,
                    Arc::new(RwLock::new(SystemGraph::default())),
                ),
                (
                    SystemStage::Render,
                    Arc::new(RwLock::new(SystemGraph::default())),
                ),
                (
                    SystemStage::Shutdown,
                    Arc::new(RwLock::new(SystemGraph::default())),
                ),
            ])),
            resources: SparseSet::default(),
            script_systems: Arc::new(RwLock::new(FxHashMap::default())),
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
        let resource = Data::new(resource, None, self.registry());
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

    pub fn read_resource<T: Component>(&self) -> anyhow::Result<MappedRwLockReadGuard<'_, T>> {
        let id = self.registry().get_static::<T>();
        let resource = self
            .resources
            .get(&id)
            .ok_or(anyhow::anyhow!("Resource does not exist"))?;
        resource.get_as().ok_or(anyhow::anyhow!(
            "Resource is not readable as a {}",
            T::type_name()
        ))
    }

    pub fn write_resource<T: Component>(&self) -> anyhow::Result<MappedRwLockWriteGuard<'_, T>> {
        let id = self.registry().get_static::<T>();
        let resource = self
            .resources
            .get(&id)
            .ok_or(anyhow::anyhow!("Resource does not exist"))?;

        resource.get_as_mut().ok_or(anyhow::anyhow!(
            "Resource is not writable as a {}",
            T::type_name()
        ))
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

    pub fn add_system<T: System>(&self, system: T) -> NodeIndex {
        self.add_system_to_stage(system, SystemStage::default())
    }

    pub fn add_system_to_stage<T: System>(&self, system: T, stage: SystemStage) -> NodeIndex {
        self.systems
            .write()
            .entry(stage)
            .or_default()
            .write()
            .add_system(system, self.components.registry())
    }

    pub fn remove_dynamic_system(&self, node: NodeIndex, stage: SystemStage) {
        self.systems
            .write()
            .entry(stage)
            .or_default()
            .write()
            .remove_system(node);
    }

    pub fn add_dynamic_system_to_stage(
        &self,
        system: DynamicSystem,
        stage: SystemStage,
    ) -> NodeIndex {
        self.systems
            .read()
            .get(&stage)
            .unwrap()
            .write()
            .add_dynamic_system(Arc::new(system))
    }

    pub fn add_script(world: &Arc<RwLock<Self>>, script_path: impl AsRef<Path>) {
        let (script, ids) = DynamicSystem::load_script(script_path, world.clone()).unwrap();
        let world_lock = world.read();
        world_lock
            .script_systems
            .write()
            .insert(script.name.clone(), (script, ids));
    }

    pub fn reload_scripts(world: &Arc<RwLock<Self>>) -> Result<(), FxHashMap<String, String>> {
        let world_lock = world.read();
        let mut scripts_lock = world_lock.script_systems.write();
        let scripts = std::mem::take(&mut *scripts_lock);
        drop(scripts_lock);

        let mut errors = FxHashMap::default();

        for (_, (script, ids)) in scripts {
            let old_systems = {
                let mut old_systems = Vec::new();
                let systems = world_lock.systems.read();
                for (stage, node) in ids.iter() {
                    if let Some(system) = systems.get(stage).unwrap().write().remove_system(*node) {
                        old_systems.push((stage.clone(), system));
                    }
                }
                old_systems
            };
            let load_result = DynamicSystem::load_script(&script.path, world.clone());
            if let Ok((script, ids)) = load_result {
                let mut scripts_lock = world_lock.script_systems.write();
                scripts_lock.insert(script.name.clone(), (script, ids));
            } else if let Err(err) = load_result {
                errors.insert(script.name.clone(), err.to_string());
                // reinsert the old version of the script so we don't lose it
                let mut scripts_lock = world_lock.script_systems.write();
                for (stage, system) in old_systems.iter() {
                    let systems = world_lock.systems.read();
                    systems
                        .get(stage)
                        .unwrap()
                        .write()
                        .add_dynamic_system(system.clone());
                }
                scripts_lock.insert(script.name.clone(), (script, ids));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn run_stage(world: &Arc<RwLock<Self>>, stage: SystemStage) -> anyhow::Result<()> {
        let world_lock = world.read();
        let systems_lock = world_lock.systems.read();
        if let Some(systems) = systems_lock.get(&stage).cloned() {
            systems
                .write()
                .autodetect_dependencies(world_lock.components.registry())?;
            drop(systems_lock);
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
