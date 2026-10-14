//! The [`World`], which owns entities, components, resources and systems.

use std::{
    any::TypeId,
    ops::{Deref, DerefMut},
    sync::atomic::AtomicU64,
};

use weaver_util::prelude::*;

use crate::{
    change_detection::{Tick, TickRange},
    component::Component,
    prelude::{
        Bundle, ComponentMap, Entities, IntoSystem, Res, ResMut, System, SystemAccess, SystemStage,
        Systems,
    },
    query::{Query, Queryable},
    removal::RemovedComponentsLog,
    system::{IntoSystemConfig, SystemParam},
};

use super::{entity::Entity, storage::Components};

/// The container for everything in a running ECS: entities, their components, resources, systems
/// and change ticks.
///
/// Almost everything takes `&self`. Component data and resources are guarded at runtime (per-type
/// borrow flags and loans), so a [`Query`] or a [`Res`]/[`ResMut`] can be held while other
/// parts of the world are used. Operations that restructure storage (spawn, destroy, insert or
/// remove components) need exclusive access to the component storage and panic if a [`Query`] is
/// still alive. Operations that change the set of systems take `&mut self`.
///
/// A world is driven either with [`World::init`], [`World::update`] and [`World::shutdown`], or by
/// running individual stages with [`World::run_stage`].
pub struct World {
    entities: SharedLock<Entities>,
    resources: Lock<ComponentMap>,
    systems: Systems,
    removed: Lock<RemovedComponentsLog>,
    change_tick: AtomicU64,
    last_change_tick: AtomicU64,
}

impl Default for World {
    fn default() -> Self {
        let components = Components::default();
        let mut resources = ComponentMap::default();
        resources
            .insert_component(components, Tick::default())
            .unwrap();

        Self {
            entities: SharedLock::new(Entities::default()),
            resources: Lock::new(resources),
            systems: Systems::default(),
            removed: Lock::new(RemovedComponentsLog::default()),
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

    /// A shared handle to the entity allocator, for code that needs to reserve ids on its own.
    pub fn entities_handle(&self) -> SharedLock<Entities> {
        self.entities.clone()
    }

    /// Flushes reserved entity ids into the allocator. See [`Entities::flush`].
    pub fn flush_entities(&self) {
        self.entities.write().flush();
    }

    /// Creates a new entity in the world with no components.
    ///
    /// Components stored for it later with [`World::insert_component`] or [`World::insert_bundle`].
    pub fn create_entity(&self) -> Entity {
        self.entities.write().alloc()
    }

    /// Marks exactly `entity` (id and generation) as alive without giving it components, e.g. to
    /// recreate an entity with a known id. See [`Entities::alloc_at`].
    pub fn insert_entity(&self, entity: Entity) {
        self.entities.write().alloc_at(entity);
    }

    /// The entity currently in slot `id`, if the slot exists. See [`Entities::find_by_id`].
    pub fn find_entity_by_id(&self, id: u32) -> Option<Entity> {
        self.entities.read().find_by_id(id)
    }

    /// Creates a new entity in the world and adds the bundle of components to it.
    ///
    /// # Panics
    ///
    /// Panics if a [`Query`] is alive.
    ///
    /// # Examples
    ///
    /// ```
    /// use weaver_ecs::prelude::*;
    ///
    /// struct Name(&'static str);
    /// struct Health(u32);
    ///
    /// let world = World::new();
    /// let player = world.spawn((Name("player"), Health(100)));
    /// assert!(world.has_component::<Health>(player));
    /// ```
    pub fn spawn<T: Bundle>(&self, bundle: T) -> Entity {
        let entity = self.create_entity();
        self.components_mut()
            .insert_bundle(entity, bundle, self.read_change_tick());
        entity
    }

    /// Destroys the entity and all its components in the world. Does nothing if the entity is
    /// already dead.
    ///
    /// # Panics
    ///
    /// Panics if a [`Query`] is alive.
    pub fn destroy_entity(&self, entity: Entity) {
        if !self.is_alive(entity) {
            return;
        }
        if let Some(types) = self.components_mut().despawn(entity) {
            let tick = self.read_change_tick();
            let mut removed = self.removed.write();
            for ty in types {
                removed.record(ty, entity, tick);
            }
        }
        self.entities.write().free(entity);
    }

    /// Returns true if the entity has been allocated (or reserved) and not destroyed since.
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.read().is_alive(entity)
    }

    /// Inserts a component into the entity, replacing any it already has. Ignored (with a
    /// warning) if the entity isn't alive, so a late insert can't resurrect a despawned entity.
    ///
    /// # Panics
    ///
    /// Panics if a [`Query`] is alive.
    pub fn insert_component<T: Component>(&self, entity: Entity, component: T) {
        self.insert_bundle(entity, (component,));
    }

    /// Inserts a bundle of components into the entity, replacing any it already has. Ignored
    /// (with a warning) if the entity isn't alive, so a late insert can't resurrect a despawned
    /// entity.
    ///
    /// # Panics
    ///
    /// Panics if a [`Query`] is alive.
    pub fn insert_bundle<T: Bundle>(&self, entity: Entity, bundle: T) {
        if !self.is_alive(entity) {
            log::warn!(
                "Ignoring insert of {} into {entity:?}, which isn't alive in this world",
                std::any::type_name::<T>()
            );
            return;
        }
        self.components_mut()
            .insert_bundle(entity, bundle, self.read_change_tick());
    }

    /// Removes a component from the entity in the world. Returns `None` if the entity doesn't
    /// exist or doesn't have the component.
    ///
    /// # Panics
    ///
    /// Panics if a [`Query`] is alive.
    pub fn remove_component<T: Component>(&self, entity: Entity) -> Option<T> {
        let removed = self.components_mut().remove_component::<T>(entity);
        if removed.is_some() {
            self.removed
                .write()
                .record(TypeId::of::<T>(), entity, self.read_change_tick());
        }
        removed
    }

    /// The entities that lost a `T` component (or were destroyed while they had one) within `ticks`.
    /// Systems should use the [`RemovedComponents`](crate::removal::RemovedComponents) parameter.
    pub fn removed_components<T: Component>(&self, ticks: TickRange) -> Vec<Entity> {
        self.removed.read().removed(TypeId::of::<T>(), ticks)
    }

    /// Ages out per-frame change trackers (currently the removed-components log). `World::update`
    /// calls this once per frame; worlds driven with `run_stage` should call it once per frame too.
    pub fn clear_trackers(&self) {
        self.removed.write().clear();
    }

    /// Checks if the entity in the world has a certain type of component.
    ///
    /// # Panics
    ///
    /// Panics if the entity has no components stored.
    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        self.components().has_component::<T>(entity)
    }

    /// Queries the world for entities with components that match the query.
    pub fn query<Q: Queryable>(&self) -> Query<Q> {
        Query::new(self)
    }

    /// Gets a shared reference to a resource from the world.
    pub fn get_resource<T: Component>(&self) -> Option<Res<T>> {
        self.get_resource_with_ticks(self.world_ticks())
    }

    /// Gets a mutable reference to a resource from the world.
    pub fn get_resource_mut<T: Component>(&self) -> Option<ResMut<T>> {
        self.get_resource_mut_with_ticks(self.world_ticks())
    }

    /// Gets a shared reference to a resource whose change detection compares against `ticks`.
    pub fn get_resource_with_ticks<T: Component>(&self, ticks: TickRange) -> Option<Res<T>> {
        self.resources
            .write()
            .get_component::<T>(ticks.last_run, ticks.this_run)
    }

    /// Gets a mutable reference to a resource whose change detection compares against (and
    /// stamps writes with) `ticks`.
    pub fn get_resource_mut_with_ticks<T: Component>(&self, ticks: TickRange) -> Option<ResMut<T>> {
        self.resources
            .write()
            .get_component_mut::<T>(ticks.last_run, ticks.this_run)
    }

    /// Borrows the component storage for reading.
    ///
    /// # Panics
    ///
    /// Panics if the storage is mutably borrowed.
    pub fn components(&self) -> Res<Components> {
        self.get_resource::<Components>().unwrap()
    }

    /// Borrows the component storage for writing.
    ///
    /// # Panics
    ///
    /// Panics if the storage is already borrowed, e.g. by a live [`Query`].
    pub fn components_mut(&self) -> ResMut<Components> {
        self.get_resource_mut::<Components>().expect(
            "Components is borrowed: entities can't be spawned, destroyed or have components inserted/removed while a Query is alive",
        )
    }

    /// Checks if the world has a certain type of resource.
    pub fn has_resource<T: Component>(&self) -> bool {
        self.resources.read().contains_component::<T>()
    }

    /// Initializes a resource in the world using its [`ConstructFromWorld`] implementation.
    /// If the resource has already been initialized, does nothing, and returns None.
    pub fn init_resource<T: Component + ConstructFromWorld>(&self) -> Option<T> {
        if self.has_resource::<T>() {
            return None;
        }
        let resource = T::from_world(self);
        self.insert_resource(resource)
    }

    /// Inserts a resource into the world.
    /// If the resource has already been inserted, replaces the existing resource with a new one, and returns the old resource.
    ///
    /// # Panics
    ///
    /// Panics if the existing resource is currently borrowed.
    pub fn insert_resource<T: Component>(&self, component: T) -> Option<T> {
        self.resources
            .write()
            .insert_component::<T>(component, self.read_change_tick())
            .unwrap()
    }

    /// Removes a resource from the world.
    ///
    /// # Panics
    ///
    /// Panics if the resource is currently borrowed.
    pub fn remove_resource<T: Component>(&self) -> Option<T> {
        self.resources.write().remove_component::<T>().unwrap()
    }

    /// Whether a stage with this label has been pushed.
    pub fn has_system_stage(&self, stage: impl SystemStage) -> bool {
        self.systems.has_stage(stage)
    }

    /// Appends a stage to the init schedule (run by [`World::init`]).
    pub fn push_init_stage(&mut self, stage: impl SystemStage) {
        self.systems.push_init_stage(stage);
    }

    /// Appends a stage to the update schedule (run by [`World::update`]).
    pub fn push_update_stage(&mut self, stage: impl SystemStage) {
        self.systems.push_update_stage(stage);
    }

    /// Appends a stage to the shutdown schedule (run by [`World::shutdown`]).
    pub fn push_shutdown_stage(&mut self, stage: impl SystemStage) {
        self.systems.push_shutdown_stage(stage);
    }

    /// Appends a stage that only runs when asked for with [`World::run_stage`].
    pub fn push_manual_stage(&mut self, stage: impl SystemStage) {
        self.systems.push_manual_stage(stage);
    }

    /// Inserts an update stage immediately before `before`.
    ///
    /// # Panics
    ///
    /// Panics if `before` isn't an update stage.
    pub fn add_update_stage_before(&mut self, stage: impl SystemStage, before: impl SystemStage) {
        self.systems.add_update_stage_before(stage, before);
    }

    /// Inserts an update stage immediately after `after`.
    ///
    /// # Panics
    ///
    /// Panics if `after` isn't an update stage.
    pub fn add_update_stage_after(&mut self, stage: impl SystemStage, after: impl SystemStage) {
        self.systems.add_update_stage_after(stage, after);
    }

    /// Adds a system to the given system stage. If the system has already been added to the stage, a warning is logged and the system is not added again.
    ///
    /// # Panics
    ///
    /// Panics if the stage hasn't been pushed.
    pub fn add_system<T, S, M>(&mut self, system: S, stage: T)
    where
        T: SystemStage,
        S: IntoSystemConfig<M>,
        M: 'static,
    {
        self.systems.add_system(system, stage);
    }

    /// Removes a system from the given stage.
    ///
    /// # Panics
    ///
    /// Panics if the stage hasn't been pushed.
    pub fn remove_system<T, S, M>(&mut self, system: S, stage: T)
    where
        T: SystemStage,
        S: IntoSystem<M>,
        M: 'static,
    {
        self.systems.remove_system(system, stage);
    }

    /// Orders two systems to run in the specified order in the given system stage.
    ///
    /// `run_first` is guaranteed to run at some point before `run_second`, but other systems can
    /// run in between them.
    ///
    /// # Panics
    ///
    /// Panics if the stage hasn't been pushed.
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
    ///
    /// # Panics
    ///
    /// Panics if the stage hasn't been pushed.
    pub fn has_system<M: 'static>(
        &self,
        system: &impl IntoSystem<M>,
        stage: impl SystemStage,
    ) -> bool {
        self.systems.has_system(system, stage)
    }

    /// Runs every system's one-time setup (parameter state, run conditions). Call after all
    /// systems are added and before the first run.
    pub fn initialize_systems(&mut self) {
        let mut systems = std::mem::take(&mut self.systems);
        systems.initialize(self);
        self.systems = systems;
    }

    /// Like [`World::initialize_systems`] for a single stage, e.g. for systems added later.
    pub fn initialize_system_stage(&mut self, stage: impl SystemStage) {
        let mut systems = std::mem::take(&mut self.systems);
        systems.initialize_stage(self, stage);
        self.systems = systems;
    }

    /// Runs the "init" system schedule once.
    ///
    /// # Errors
    ///
    /// Returns the first error a system returns, and stops.
    pub fn init(&mut self) -> Result<()> {
        // FIXME: if a system panics or returns Err, `self.systems` stays the empty default and
        // the World loses all its stages (same in update, shutdown and run_stage).
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_init(self)?;
        self.systems = systems;
        Ok(())
    }

    /// Runs the "update" system schedule once, then advances the change tick and ages out the
    /// removed-components log.
    ///
    /// # Errors
    ///
    /// Returns the first error a system returns, and stops.
    pub fn update(&mut self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_update(self)?;
        self.systems = systems;
        self.increment_change_tick();
        self.clear_trackers();
        Ok(())
    }

    /// Runs the "shutdown" system schedule once.
    ///
    /// # Errors
    ///
    /// Returns the first error a system returns, and stops.
    pub fn shutdown(&mut self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_shutdown(self)?;
        self.systems = systems;
        Ok(())
    }

    /// Runs the given system stage once.
    ///
    /// # Errors
    ///
    /// Returns the first error a system returns, and stops.
    ///
    /// # Panics
    ///
    /// Panics if the stage hasn't been pushed.
    pub fn run_stage(&mut self, stage: impl SystemStage) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_stage(self, stage)?;
        self.systems = systems;
        Ok(())
    }

    /// The ticks that code running outside of a system (direct `World` access) compares against:
    /// changes since the last [`World::increment_change_tick`].
    pub fn world_ticks(&self) -> TickRange {
        TickRange::new(self.last_change_tick(), self.read_change_tick())
    }

    /// Advances the change tick and returns the tick from before the advance, without touching
    /// [`World::last_change_tick`]. The scheduler calls this before every system run and uses the
    /// returned tick as the run's `this_run`: every run gets its own tick, and anything written
    /// after the run (by later systems, commands, or direct world access) is stamped with a
    /// strictly newer tick, so the system sees it next time.
    pub fn bump_change_tick(&self) -> Tick {
        Tick(
            self.change_tick
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        )
    }

    /// Advances the change tick and sets [`World::last_change_tick`] to the previous one.
    /// `World::update` calls this at the end of a frame; it only affects direct `World` access,
    /// since systems track their own last-run tick.
    pub fn increment_change_tick(&self) -> Tick {
        let tick = self
            .change_tick
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.last_change_tick
            .store(tick, std::sync::atomic::Ordering::Relaxed);
        Tick(tick)
    }

    /// The current change tick.
    pub fn read_change_tick(&self) -> Tick {
        Tick(self.change_tick.load(std::sync::atomic::Ordering::Relaxed))
    }

    /// The tick that was current when [`World::increment_change_tick`] last ran.
    pub fn last_change_tick(&self) -> Tick {
        Tick(
            self.last_change_tick
                .load(std::sync::atomic::Ordering::Relaxed),
        )
    }
}

/// A type that can build itself from the world, e.g. a resource that needs to look at other
/// resources when it's created.
///
/// Every `Default` type implements it by calling `default()`.
pub trait ConstructFromWorld {
    /// Creates the value, reading whatever it needs from `world`.
    fn from_world(world: &World) -> Self;
}

impl<T: Default> ConstructFromWorld for T {
    fn from_world(_: &World) -> Self {
        T::default()
    }
}

/// A system parameter that builds a `T` with [`ConstructFromWorld`] each time the system runs.
/// Derefs to the `T`.
pub struct FromWorld<T: ConstructFromWorld> {
    value: T,
}

impl<T: ConstructFromWorld> FromWorld<T> {
    /// Builds the value from `world`.
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

    fn fetch(world: &World, _state: &Self::State, _ticks: TickRange) -> Self::Item {
        Self::new(world)
    }
}
