use std::{
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
};

use weaver_util::{
    lock::{ArcRead, ArcWrite, Lock, SharedLock},
    prelude::Result,
};

use crate::prelude::{
    Bundle, IntoSystem, Query, QueryBuilder, QueryFetch, QueryFilter, Res, ResMut, Resource,
    Resources, SystemSchedule, SystemStage, Tick,
};

use super::{
    component::Component,
    entity::Entity,
    storage::{Mut, Ref, Storage},
};

#[derive(Clone, Default)]
pub struct WorldLock(SharedLock<World>);

impl WorldLock {
    pub fn read(&self) -> ReadWorld {
        ReadWorld(self.0.read_arc())
    }

    pub fn write(&self) -> WriteWorld {
        WriteWorld(self.0.write_arc())
    }

    pub fn run_stage<S: SystemStage>(&self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.write().systems);
        systems.run_stage::<S>(self)?;
        self.write().systems = systems;
        Ok(())
    }

    pub fn init(&self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.write().systems);
        systems.run_init(self)?;
        self.write().systems = systems;
        Ok(())
    }

    pub fn update(&self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.write().systems);
        systems.run_update(self)?;
        self.write().systems = systems;
        Ok(())
    }

    pub fn shutdown(&self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.write().systems);
        systems.run_shutdown(self)?;
        self.write().systems = systems;
        Ok(())
    }

    pub fn get_resource<T: Resource>(&self) -> Option<Res<T>> {
        self.read().get_resource::<T>()
    }

    pub fn get_resource_mut<T: Resource>(&self) -> Option<ResMut<T>> {
        self.read().get_resource_mut::<T>()
    }

    pub fn has_resource<T: Resource>(&self) -> bool {
        self.read().has_resource::<T>()
    }

    pub fn insert_resource<T: Resource>(&self, component: T) {
        self.write().insert_resource(component)
    }

    pub fn remove_resource<T: Resource>(&self) -> Option<T> {
        self.write().remove_resource::<T>()
    }

    pub fn spawn<T: Bundle>(&self, bundle: T) -> Entity {
        self.write().spawn(bundle)
    }

    pub fn insert_component<T: Component>(&self, entity: Entity, component: T) {
        self.write().insert_component(entity, component)
    }

    pub fn insert_components<T: Bundle>(&self, entity: Entity, bundle: T) {
        self.write().insert_components(entity, bundle)
    }

    pub fn get_component<T: Component>(&self, entity: Entity) -> Option<Ref<T>> {
        self.read().get_component::<T>(entity)
    }

    pub fn get_component_mut<T: Component>(&self, entity: Entity) -> Option<Mut<T>> {
        self.read().get_component_mut::<T>(entity)
    }

    pub fn query<Q: QueryFetch>(&self) -> Query<Q, ()> {
        self.read().query()
    }

    pub fn query_filtered<Q: QueryFetch, F: QueryFilter>(&self) -> Query<Q, F> {
        self.read().query_filtered()
    }
}

pub struct ReadWorld(ArcRead<World>);

impl Deref for ReadWorld {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct WriteWorld(ArcWrite<World>);

impl Deref for WriteWorld {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WriteWorld {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct World {
    root_scene_entity: Entity,
    next_entity: AtomicU32,
    free_entities: Lock<Vec<Entity>>,
    storage: Storage,
    resources: Resources,
    change_tick: AtomicU64,
    last_change_tick: Tick,
    systems: SystemSchedule,
}

impl Default for World {
    fn default() -> Self {
        Self {
            root_scene_entity: Entity::new(1, 0),
            next_entity: AtomicU32::new(0),
            free_entities: Lock::new(Vec::new()),
            storage: Storage::new(),
            resources: Resources::default(),
            change_tick: AtomicU64::new(0),
            last_change_tick: Tick::new(0),
            systems: SystemSchedule::default(),
        }
    }
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_world_lock(self) -> WorldLock {
        WorldLock(SharedLock::new(self))
    }

    pub fn storage(&self) -> &Storage {
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

    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> Entity {
        let entity = self.create_entity();
        self.storage
            .insert_components(entity, bundle, self.read_change_tick());
        entity
    }

    pub fn destroy_entity(&mut self, entity: Entity) {
        self.storage.remove_entity(entity);
        self.free_entities.write().push(entity);
    }

    pub fn insert_component<T: Component>(&mut self, entity: Entity, component: T) {
        self.storage
            .insert_component(entity, component, self.read_change_tick())
    }

    pub fn insert_components<T: Bundle>(&mut self, entity: Entity, bundle: T) {
        self.storage
            .insert_components(entity, bundle, self.read_change_tick())
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> Option<T> {
        self.storage.remove_component::<T>(entity)
    }

    pub fn get_component<T: Component>(&self, entity: Entity) -> Option<Ref<T>> {
        self.storage
            .get_component::<T>(entity, self.last_change_tick(), self.read_change_tick())
    }

    pub fn get_component_mut<T: Component>(&self, entity: Entity) -> Option<Mut<T>> {
        self.storage.get_component_mut::<T>(
            entity,
            self.last_change_tick(),
            self.read_change_tick(),
        )
    }

    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        self.storage.has_component::<T>(entity)
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
        self.resources.get::<T>()
    }

    pub fn get_resource_mut<T: Resource>(&self) -> Option<ResMut<T>> {
        self.resources
            .get_mut::<T>(self.last_change_tick, self.read_change_tick())
    }

    pub fn has_resource<T: Resource>(&self) -> bool {
        self.resources.contains::<T>()
    }

    pub fn insert_resource<T: Resource>(&mut self, component: T) {
        self.resources.insert(component, self.read_change_tick())
    }

    pub fn remove_resource<T: Resource>(&mut self) -> Option<T> {
        self.resources.remove::<T>().map(|(resource, _)| resource)
    }

    pub fn increment_change_tick(&mut self) {
        self.last_change_tick = Tick::new(self.change_tick.fetch_add(1, Ordering::AcqRel));
    }

    pub fn read_change_tick(&self) -> Tick {
        Tick::new(self.change_tick.load(Ordering::Acquire))
    }

    pub fn change_tick(&mut self) -> Tick {
        Tick::new(*self.change_tick.get_mut())
    }

    pub fn last_change_tick(&self) -> Tick {
        self.last_change_tick
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
        system: impl IntoSystem<M> + 'static,
        stage: S,
    ) {
        self.systems.add_system(system, stage);
    }

    pub fn add_system_before<S: SystemStage, M1, M2>(
        &mut self,
        system: impl IntoSystem<M1> + 'static,
        before: impl IntoSystem<M2> + 'static,
        stage: S,
    ) {
        self.systems.add_system_before(system, before, stage);
    }

    pub fn add_system_after<S: SystemStage, M1, M2>(
        &mut self,
        system: impl IntoSystem<M1> + 'static,
        after: impl IntoSystem<M2> + 'static,
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
