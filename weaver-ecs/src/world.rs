use std::{path::Path, sync::Arc};

use parking_lot::{MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock};
use petgraph::prelude::NodeIndex;
use rustc_hash::FxHashMap;

use crate::{
    bundle::Bundle,
    component::Data,
    entity::Entity,
    prelude::{Component, EntityGraph},
    query::{DynamicQueryBuilder, Query, QueryFilter, Queryable},
    registry::DynamicId,
    script::Script,
    storage::{Components, SparseSet},
    system::{DynamicSystem, System},
    system::{SystemGraph, SystemStage},
};

#[allow(clippy::type_complexity)]
pub struct World {
    pub components: Components,
    pub systems: RwLock<FxHashMap<SystemStage, Arc<RwLock<SystemGraph>>>>,
    pub resources: SparseSet<DynamicId, Data>,
    pub script_systems: Arc<RwLock<FxHashMap<String, (Script, Vec<(SystemStage, NodeIndex)>)>>>,
    pub system_errors: Arc<RwLock<FxHashMap<String, String>>>,
}

impl World {
    pub fn new() -> Self {
        let mut this = Self {
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
                    SystemStage::PostRender,
                    Arc::new(RwLock::new(SystemGraph::default())),
                ),
                (
                    SystemStage::Shutdown,
                    Arc::new(RwLock::new(SystemGraph::default())),
                ),
            ])),
            resources: SparseSet::default(),
            script_systems: Arc::new(RwLock::new(FxHashMap::default())),
            system_errors: Arc::new(RwLock::new(FxHashMap::default())),
        };

        this.add_resource(EntityGraph::default()).unwrap();
        this
    }

    pub fn create_entity(&mut self) -> Entity {
        let entity = self.components.create_entity();
        self.write_resource::<EntityGraph>()
            .unwrap()
            .add_entity(entity);
        entity
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

    pub fn add_relation(&mut self, parent: &Entity, child: &Entity) -> bool {
        let mut graph = self.write_resource::<EntityGraph>().unwrap();
        graph.add_relation(*parent, *child)
    }

    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> Entity {
        let entity = bundle.build(&mut self.components);
        self.write_resource::<EntityGraph>()
            .unwrap()
            .add_entity(entity);
        entity
    }

    pub fn spawn_with_children<T: Bundle>(
        &mut self,
        bundle: T,
        add_children: impl FnOnce(&mut Self) -> Vec<Entity>,
    ) -> Entity {
        let parent = self.spawn(bundle);
        let children = add_children(self);
        for child in children {
            self.add_relation(&parent, &child);
        }
        parent
    }

    pub fn despawn(&mut self, entity: Entity) {
        self.components.despawn(entity.id());

        let mut graph = self.write_resource::<EntityGraph>().unwrap();
        graph.remove_entity(entity);
    }

    pub fn despawn_recursive(&mut self, entity: Entity) {
        let graph = self.write_resource::<EntityGraph>().unwrap();
        let mut children = graph.get_all_children(entity);
        drop(graph);
        children.push(entity);

        for child in children {
            self.despawn(child);
            self.write_resource::<EntityGraph>()
                .unwrap()
                .remove_entity(child);
        }
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

    pub fn add_system<T: System + 'static>(&self, system: T) -> NodeIndex {
        self.add_system_to_stage(system, SystemStage::default())
    }

    pub fn add_system_to_stage<T: System + 'static>(
        &self,
        system: T,
        stage: SystemStage,
    ) -> NodeIndex {
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

    pub fn system_errors(&self) -> FxHashMap<String, String> {
        self.system_errors.read().clone()
    }

    pub fn add_script(world: &Arc<RwLock<Self>>, script_path: impl AsRef<Path>) {
        let (script, ids) = DynamicSystem::load_script(script_path, world.clone()).unwrap();
        let world_lock = world.read();
        world_lock
            .script_systems
            .write()
            .insert(script.name.clone(), (script, ids));
    }

    pub fn reload_scripts(world: &Arc<RwLock<Self>>) {
        let world_lock = world.read();

        world_lock.system_errors.write().clear();

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
                        old_systems.push((*stage, system));
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

        if !errors.is_empty() {
            let world_lock = world.read();
            world_lock.system_errors.write().extend(errors);
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
            systems.write().run(world)?;

            for (sys_name, error) in systems.read().errors() {
                world
                    .write()
                    .system_errors
                    .write()
                    .insert(sys_name.to_owned(), error.to_owned());
            }
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

    pub fn components_iter(&self, entity: &Entity) -> impl Iterator<Item = &Data> {
        self.components.entity_components_iter(entity.id()).unwrap()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
