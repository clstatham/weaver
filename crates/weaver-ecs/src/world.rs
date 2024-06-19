use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use weaver_util::{lock::Lock, prelude::Result};

use crate::prelude::{
    Bundle, FunctionSystem, Query, QueryBuilder, QueryFetch, QueryFilter, Res, ResMut, Resource,
    Resources, SystemSchedule, SystemStage, Tick,
};

use super::{
    component::Component,
    entity::Entity,
    storage::{Mut, Ref, Storage},
};

pub struct World {
    root_scene_entity: Entity,
    next_entity: AtomicU32,
    free_entities: Lock<Vec<Entity>>,
    storage: Lock<Storage>,
    resources: Lock<Resources>,
    update_tick: AtomicU64,
    systems: SystemSchedule,
}

impl Default for World {
    fn default() -> Self {
        Self {
            root_scene_entity: Entity::new(1, 0),
            next_entity: AtomicU32::new(0),
            free_entities: Lock::new(Vec::new()),
            storage: Lock::new(Storage::new()),
            resources: Lock::new(Resources::default()),
            update_tick: AtomicU64::new(0),
            systems: SystemSchedule::default(),
        }
    }
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn storage(&self) -> &Lock<Storage> {
        &self.storage
    }

    pub fn create_entity(&self) -> Entity {
        if let Some(entity) = self.free_entities.write().pop() {
            Entity::new(entity.id(), entity.generation() + 1)
        } else {
            let id = self.next_entity.fetch_add(1, Ordering::Relaxed);
            Entity::new(id, 0)
        }
    }

    pub fn spawn<T: Bundle>(&self, bundle: T) -> Entity {
        let entity = self.create_entity();
        self.storage().write().insert_components(entity, bundle);
        entity
    }

    pub fn destroy_entity(&self, entity: Entity) {
        self.storage.write().remove_entity(entity);
        self.free_entities.write().push(entity);
    }

    pub fn insert_component<T: Component>(&self, entity: Entity, component: T) {
        self.storage.write().insert_component(entity, component)
    }

    pub fn insert_components<T: Bundle>(&self, entity: Entity, bundle: T) {
        self.storage.write().insert_components(entity, bundle)
    }

    pub fn remove_component<T: Component>(&self, entity: Entity) -> Option<T> {
        self.storage.write().remove_component::<T>(entity)
    }

    pub fn get_component<T: Component>(&self, entity: Entity) -> Option<Ref<T>> {
        self.storage.read().get_component::<T>(entity)
    }

    pub fn get_component_mut<T: Component>(&self, entity: Entity) -> Option<Mut<T>> {
        self.storage.read().get_component_mut::<T>(entity)
    }

    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        self.storage.read().has_component::<T>(entity)
    }

    pub fn query<Q: QueryFetch>(&self) -> Query<Q, ()> {
        Query::new(self)
    }

    pub fn query_filtered<Q: QueryFetch, F: QueryFilter>(&self) -> Query<Q, F> {
        Query::new(self)
    }

    pub fn query_builder(&self) -> QueryBuilder {
        QueryBuilder::new(self)
    }

    pub const fn root_scene_entity(&self) -> Entity {
        self.root_scene_entity
    }

    pub fn get_resource<T: Resource>(&self) -> Option<Res<T>> {
        self.resources.read().get::<T>()
    }

    pub fn get_resource_mut<T: Resource>(&self) -> Option<ResMut<T>> {
        self.resources.read().get_mut::<T>()
    }

    pub fn has_resource<T: Resource>(&self) -> bool {
        self.resources.read().contains::<T>()
    }

    pub fn insert_resource<T: Resource>(&self, component: T) {
        self.resources.write().insert(component)
    }

    pub fn remove_resource<T: Resource>(&self) -> Option<T> {
        self.resources.write().remove::<T>()
    }

    pub fn update_tick(&self) -> Tick {
        Tick::new(self.update_tick.load(Ordering::Acquire))
    }

    pub fn run_stage<S: SystemStage>(&mut self) -> Result<()> {
        let systems = std::mem::take(&mut self.systems);
        systems.run_stage::<S>(self)?;
        self.systems = systems;
        Ok(())
    }

    pub fn init(&mut self) -> Result<()> {
        let systems = std::mem::take(&mut self.systems);
        systems.run_init(self)?;
        self.systems = systems;
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        self.update_tick.fetch_add(1, Ordering::AcqRel);
        let systems = std::mem::take(&mut self.systems);
        systems.run_update(self)?;
        self.systems = systems;
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<()> {
        let systems = std::mem::take(&mut self.systems);
        systems.run_shutdown(self)?;
        self.systems = systems;
        Ok(())
    }

    pub fn push_init_stage<T: SystemStage>(&mut self) {
        self.systems.push_init_stage::<T>();
    }

    pub fn push_update_stage<T: SystemStage>(&mut self) {
        self.systems.push_update_stage::<T>();
    }

    pub fn push_shutdown_stage<T: SystemStage>(&mut self) {
        self.systems.push_shutdown_stage::<T>();
    }

    pub fn push_manual_stage<T: SystemStage>(&mut self) {
        self.systems.push_manual_stage::<T>();
    }

    pub fn add_stage_before<T: SystemStage, U: SystemStage>(&mut self) {
        self.systems.add_stage_before::<T, U>();
    }

    pub fn add_stage_after<T: SystemStage, U: SystemStage>(&mut self) {
        self.systems.add_stage_after::<T, U>();
    }

    pub fn add_system<S: SystemStage, M>(
        &mut self,
        system: impl FunctionSystem<M> + 'static,
        stage: S,
    ) {
        self.systems.add_system(system, stage);
    }

    pub fn add_system_before<S: SystemStage, M1, M2>(
        &mut self,
        system: impl FunctionSystem<M1> + 'static,
        before: impl FunctionSystem<M2> + 'static,
        stage: S,
    ) {
        self.systems.add_system_before(system, before, stage);
    }

    pub fn add_system_after<S: SystemStage, M1, M2>(
        &mut self,
        system: impl FunctionSystem<M1> + 'static,
        after: impl FunctionSystem<M2> + 'static,
        stage: S,
    ) {
        self.systems.add_system_after(system, after, stage);
    }
}

pub trait FromWorld {
    fn from_world(world: &World) -> Self;
}

impl<T: Default> FromWorld for T {
    fn from_world(_: &World) -> Self {
        T::default()
    }
}
