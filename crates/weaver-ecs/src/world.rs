use std::{
    ops::{Deref, DerefMut},
    sync::atomic::AtomicU64,
};

use weaver_util::{prelude::*, span};

use crate::{
    component::Component,
    prelude::{
        Bundle, Command, Commands, ComponentMap, Entities, IntoSystem, Res, ResMut, System,
        SystemAccess, SystemStage, Systems,
    },
    query::{Query, Queryable},
    system::{IntoSystemConfig, SystemParam},
};

use super::{entity::Entity, storage::Components};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(u64);

impl Tick {
    pub const MAX: Self = Self(u64::MAX);

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn is_newer_than(&self, last_run: Tick, this_run: Tick) -> bool {
        let last_diff = this_run.relative_to(last_run).as_u64();
        let this_diff = this_run.relative_to(*self).as_u64();

        this_diff < last_diff
    }

    pub fn relative_to(&self, other: Tick) -> Tick {
        Tick(self.0.wrapping_sub(other.0))
    }
}

pub struct WorldTicks {
    pub last_change_tick: Tick,
    pub change_tick: Tick,
}

impl SystemParam for WorldTicks {
    type Item = WorldTicks;
    type State = ();

    fn access() -> SystemAccess {
        SystemAccess::default()
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _state: &Self::State) -> Self {
        WorldTicks {
            last_change_tick: world.last_change_tick(),
            change_tick: world.read_change_tick(),
        }
    }
}

pub struct World {
    entities: Lock<Entities>,
    resources: Lock<ComponentMap>,
    systems: Systems,
    command_tx: async_channel::Sender<Command>,
    command_rx: async_channel::Receiver<Command>,
    change_tick: AtomicU64,
    last_change_tick: AtomicU64,
}

impl Default for World {
    fn default() -> Self {
        let components = Components::default();
        let mut resources = ComponentMap::default();
        resources.insert_component(components).unwrap();

        let (command_tx, command_rx) = async_channel::unbounded();

        Self {
            entities: Lock::new(Entities::default()),
            resources: Lock::new(resources),
            systems: Systems::default(),
            command_tx,
            command_rx,
            change_tick: AtomicU64::new(0),
            last_change_tick: AtomicU64::new(0),
        }
    }
}

impl World {
    /// Creates a new world.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn commands(&self) -> Commands {
        Commands {
            tx: self.command_tx.clone(),
        }
    }

    pub fn apply_commands(&mut self) {
        let span = span!(DEBUG, "apply_commands");
        let _span = span.enter();
        while let Ok(command) = self.command_rx.try_recv() {
            command.run(self);
        }
    }

    /// Creates a new entity in the world.
    pub fn create_entity(&self) -> Entity {
        self.entities.write().alloc()
    }

    pub fn insert_entity(&self, entity: Entity) {
        self.entities.write().alloc_at(entity);
    }

    pub fn find_entity_by_id(&self, id: u32) -> Option<Entity> {
        self.entities.read().find_by_id(id)
    }

    /// Creates a new entity in the world and adds the bundle of components to it.
    pub fn spawn<T: Bundle>(&self, bundle: T) -> Entity {
        let entity = self.create_entity();
        self.components_mut().insert_bundle(entity, bundle);
        entity
    }

    /// Destroys the entity and all its components in the world.
    pub fn destroy_entity(&self, entity: Entity) {
        self.components_mut().remove_entity(entity);
        self.entities.write().free(entity);
    }

    pub fn insert_component<T: Component>(&self, entity: Entity, component: T) {
        self.components_mut().insert_bundle(entity, (component,));
    }

    /// Inserts a bundle of components into the entity in the world.
    pub fn insert_bundle<T: Bundle>(&self, entity: Entity, bundle: T) {
        self.components_mut().insert_bundle(entity, bundle);
    }

    /// Removes a component from the entity in the world.
    pub fn remove_component<T: Component>(&self, entity: Entity) -> Option<T> {
        self.components_mut().remove_component::<T>(entity)
    }

    /// Checks if the entity in the world has a certain type of component.
    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        self.components().has_component::<T>(entity)
    }

    /// Queries the world for entities with components that match the query.
    pub fn query<Q: Queryable>(&self) -> Query<Q> {
        Query::new(self)
    }

    /// Gets a shared reference to a resource from the world.
    pub fn get_resource<T: Component>(&self) -> Option<Res<T>> {
        self.resources.write().get_component::<T>().map(Res)
    }

    /// Gets a mutable reference to a resource from the world.
    pub fn get_resource_mut<T: Component>(&self) -> Option<ResMut<T>> {
        self.resources.write().get_component_mut::<T>().map(ResMut)
    }

    pub fn components(&self) -> Res<Components> {
        self.get_resource::<Components>().unwrap()
    }

    pub fn components_mut(&self) -> ResMut<Components> {
        self.get_resource_mut::<Components>().unwrap()
    }

    /// Checks if the world has a certain type of resource.
    pub fn has_resource<T: Component>(&self) -> bool {
        self.resources.read().contains_component::<T>()
    }

    /// Initializes a resource in the world. The resource is initialized using its implementation of `FromWorld`.
    /// If the resource has already been initialized, replaces the existing resource with a new one, and returns the old resource.
    pub fn init_resource<T: Component + ConstructFromWorld>(&self) -> Option<T> {
        let resource = T::from_world(self);
        self.insert_resource(resource)
    }

    /// Inserts a resource into the world.
    /// If the resource has already been inserted, replaces the existing resource with a new one, and returns the old resource.
    pub fn insert_resource<T: Component>(&self, component: T) -> Option<T> {
        self.resources
            .write()
            .insert_component::<T>(component)
            .unwrap()
    }

    /// Removes a resource from the world.
    pub fn remove_resource<T: Component>(&self) -> Option<T> {
        self.resources.write().remove_component::<T>().unwrap()
    }

    pub fn has_system_stage(&self, stage: impl SystemStage) -> bool {
        self.systems.has_stage(stage)
    }

    pub fn push_init_stage(&mut self, stage: impl SystemStage) {
        self.systems.push_init_stage(stage);
    }

    pub fn push_update_stage(&mut self, stage: impl SystemStage) {
        self.systems.push_update_stage(stage);
    }

    pub fn push_shutdown_stage(&mut self, stage: impl SystemStage) {
        self.systems.push_shutdown_stage(stage);
    }

    pub fn push_manual_stage(&mut self, stage: impl SystemStage) {
        self.systems.push_manual_stage(stage);
    }

    pub fn add_update_stage_before(&mut self, stage: impl SystemStage, before: impl SystemStage) {
        self.systems.add_update_stage_before(stage, before);
    }

    pub fn add_update_stage_after(&mut self, stage: impl SystemStage, after: impl SystemStage) {
        self.systems.add_update_stage_after(stage, after);
    }

    /// Adds a system to the given system stage. If the system has already been added to the stage, a warning is logged and the system is not added again.
    pub fn add_system<T, S, M>(&mut self, system: S, stage: T)
    where
        T: SystemStage,
        S: IntoSystemConfig<M>,
        M: 'static,
    {
        self.systems.add_system(system, stage);
    }

    /// Orders two systems to run in the specified order in the given system stage.
    ///
    /// Note that this doesn't necessarily mean the systems will run in this exact sequence; `run_first` is guaranteed to run *at some point* before `run_second`, but there might be other systems that run in between them.
    pub fn order_systems<Stage, M1, M2, S1, S2>(
        &mut self,
        run_first: S1,
        run_second: S2,
        stage: Stage,
    ) where
        Stage: SystemStage,
        M1: 'static,
        M2: 'static,
        S1: IntoSystem<M1>,
        S2: IntoSystem<M2>,
        S1::System: System,
        S2::System: System,
    {
        self.systems.order_systems(run_first, run_second, stage);
    }

    /// Checks if the system has been added to the given system stage.
    pub fn has_system<M: 'static>(
        &self,
        system: &impl IntoSystem<M>,
        stage: impl SystemStage,
    ) -> bool {
        self.systems.has_system(system, stage)
    }

    pub fn initialize_systems(&mut self) {
        let mut systems = std::mem::take(&mut self.systems);
        systems.initialize(self);
        self.systems = systems;
    }

    pub fn initialize_system_stage(&mut self, stage: impl SystemStage) {
        let mut systems = std::mem::take(&mut self.systems);
        systems.initialize_stage(self, stage);
        self.systems = systems;
    }

    /// Runs the "init" system schedule once.
    pub async fn init(&mut self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_init(self).await?;
        self.systems = systems;
        Ok(())
    }

    /// Runs the "update" system schedule once.
    pub async fn update(&mut self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_update(self).await?;
        self.systems = systems;
        Ok(())
    }

    /// Runs the "shutdown" system schedule once.
    pub async fn shutdown(&mut self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_shutdown(self).await?;
        self.systems = systems;
        Ok(())
    }

    /// Runs the given system stage once.
    pub async fn run_stage(&mut self, stage: impl SystemStage) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_stage(self, stage).await?;
        self.systems = systems;
        Ok(())
    }

    pub fn increment_change_tick(&self) -> Tick {
        let tick = self
            .change_tick
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.last_change_tick
            .store(tick, std::sync::atomic::Ordering::Relaxed);
        Tick(tick)
    }

    pub fn read_change_tick(&self) -> Tick {
        Tick(self.change_tick.load(std::sync::atomic::Ordering::Relaxed))
    }

    pub fn last_change_tick(&self) -> Tick {
        Tick(
            self.last_change_tick
                .load(std::sync::atomic::Ordering::Relaxed),
        )
    }
}

/// A trait for creating a new instance of a type from a world.
pub trait ConstructFromWorld {
    fn from_world(world: &World) -> Self;
}

impl<T: Default> ConstructFromWorld for T {
    fn from_world(_: &World) -> Self {
        T::default()
    }
}

pub struct FromWorld<T: ConstructFromWorld> {
    value: T,
}

impl<T: ConstructFromWorld> FromWorld<T> {
    pub fn new(world: &World) -> Self {
        Self {
            value: T::from_world(world),
        }
    }
}

impl<T: ConstructFromWorld> Deref for FromWorld<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: ConstructFromWorld> DerefMut for FromWorld<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T> SystemParam for FromWorld<T>
where
    T: ConstructFromWorld + Send + Sync + 'static,
{
    type Item = FromWorld<T>;
    type State = ();

    fn access() -> SystemAccess {
        SystemAccess::default()
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _state: &Self::State) -> Self::Item {
        Self::new(world)
    }
}
