use std::{
    cell::UnsafeCell,
    marker::PhantomData,
    sync::atomic::{AtomicU64, Ordering},
};

use weaver_util::{warn_once, Result};

use crate::prelude::{
    Bundle, Entities, IntoSystem, MultiResource, QueryFetch, QueryFilter, QueryState, Res, ResMut,
    Resource, Resources, System, SystemStage, Systems, Tick,
};

use super::{
    component::Component,
    entity::Entity,
    storage::{Mut, Ref, Storage},
};

#[derive(Clone, Copy)]
pub struct UnsafeWorldCell<'a>(
    *mut World,
    PhantomData<&'a World>,
    PhantomData<&'a UnsafeCell<World>>,
);

impl<'a> UnsafeWorldCell<'a> {
    /// Creates a new `UnsafeWorldCell` from a mutable reference to the world.
    /// This is the correct way to create an `UnsafeWorldCell` for exclusive access to the world.
    ///
    /// # Safety
    ///
    /// Callers must ensure that no other references to the world are held for the lifetime of the `UnsafeWorldCell`.
    pub fn new_exclusive(world: &'a mut World) -> Self {
        Self(world as *mut _, PhantomData, PhantomData)
    }

    /// Creates a new `UnsafeWorldCell` from a shared reference to the world.
    /// This is the correct way to create an `UnsafeWorldCell` for shared (read-only) access to the world.
    ///
    /// The `UnsafeWorldCell` created from this function must NOT be used to access the world mutably.
    ///
    /// # Safety
    ///
    /// Callers mut ensure that no mutable references to the world are held for the lifetime of the `UnsafeWorldCell`,
    /// and that the world is not accessed mutably for the lifetime of the `UnsafeWorldCell`.
    pub fn new_shared(world: &'a World) -> Self {
        Self(world as *const _ as *mut _, PhantomData, PhantomData)
    }

    /// # Safety
    ///
    /// This function is unsafe because it dereferences the pointer to the world.
    /// Callers must ensure that the world is not accessed concurrently, and that no mutable references to the world are held.
    pub unsafe fn world(self) -> &'a World {
        unsafe { &*self.0 }
    }

    /// # Safety
    ///
    /// This function is unsafe because it dereferences the pointer to the world.
    /// Callers must ensure that the world is not accessed concurrently, and that no other references to the world are held.
    pub unsafe fn world_mut(self) -> &'a mut World {
        unsafe { &mut *self.0 }
    }

    /// # Safety
    ///
    /// This function is unsafe because it dereferences the pointer to the world.
    /// Callers must ensure that the world is not accessed concurrently, and that no mutable references to the world are held.
    pub unsafe fn entities(self) -> &'a Entities {
        unsafe { self.world().entities() }
    }

    /// # Safety
    ///
    /// This function is unsafe because it dereferences the pointer to the world.
    /// Callers must ensure that the world is not accessed concurrently, and that no mutable references to the world are held.
    pub unsafe fn entities_mut(self) -> &'a mut Entities {
        unsafe { self.world_mut().entities_mut() }
    }

    /// # Safety
    ///
    /// This function is unsafe because it dereferences the pointer to the world.
    pub unsafe fn get_resource<T: Resource>(self) -> Option<Res<'a, T>> {
        unsafe { self.world().get_resource::<T>() }
    }

    /// # Safety
    ///
    /// This function is unsafe because it dereferences the pointer to the world.
    pub unsafe fn get_resource_mut<T: Resource>(self) -> Option<ResMut<'a, T>> {
        unsafe { self.world_mut().get_resource_mut::<T>() }
    }

    pub fn read_change_tick(self) -> Tick {
        unsafe { self.world().read_change_tick() }
    }

    pub fn last_change_tick(self) -> Tick {
        unsafe { self.world().last_change_tick() }
    }
}

pub struct World {
    entities: Entities,
    storage: Storage,
    resources: Resources,
    change_tick: AtomicU64,
    last_change_tick: Tick,
    systems: Systems,
}

unsafe impl Send for World {}
unsafe impl Sync for World {}

impl Default for World {
    fn default() -> Self {
        Self {
            entities: Entities::default(),
            storage: Storage::new(),
            resources: Resources::default(),
            change_tick: AtomicU64::new(0),
            last_change_tick: Tick::new(0),
            systems: Systems::default(),
        }
    }
}

impl World {
    /// Creates a new world.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an `UnsafeWorldCell` for shared (read-only) access to the world.
    /// This is the correct way to create an `UnsafeWorldCell` for shared (read-only) access to the world.
    /// The `UnsafeWorldCell` created from this function must NOT be used to access the world mutably.
    ///
    /// # Safety
    ///
    /// Callers mut ensure that no mutable references to the world are held for the lifetime of the `UnsafeWorldCell`,
    /// and that the world is not accessed mutably for the lifetime of the `UnsafeWorldCell`.
    pub fn as_unsafe_world_cell_readonly(&self) -> UnsafeWorldCell {
        UnsafeWorldCell::new_shared(self)
    }

    /// Creates an `UnsafeWorldCell` for exclusive access to the world.
    /// This is the correct way to create an `UnsafeWorldCell` for exclusive access to the world.
    ///
    /// # Safety
    ///
    /// Callers must ensure that no other references to the world are held for the lifetime of the `UnsafeWorldCell`.
    pub fn as_unsafe_world_cell(&mut self) -> UnsafeWorldCell {
        UnsafeWorldCell::new_exclusive(self)
    }

    /// Returns a reference to the world's component storage.
    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    /// Returns a mutable reference to the world's component storage.
    pub fn storage_mut(&mut self) -> &mut Storage {
        &mut self.storage
    }

    pub fn entities(&self) -> &Entities {
        &self.entities
    }

    pub fn entities_mut(&mut self) -> &mut Entities {
        &mut self.entities
    }

    /// Creates a new entity in the world.
    pub fn create_entity(&mut self) -> Entity {
        self.entities.alloc()
    }

    pub fn insert_entity(&mut self, entity: Entity) {
        self.entities.alloc_at(entity);
    }

    pub fn find_entity_by_id(&self, id: u32) -> Option<Entity> {
        self.entities.find_by_id(id)
    }

    /// Creates a new entity in the world and adds the bundle of components to it.
    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> Entity {
        let entity = self.create_entity();
        let change_tick = self.change_tick();
        self.storage.insert_bundle(entity, bundle, change_tick);
        entity
    }

    /// Destroys the entity and all its components in the world.
    pub fn destroy_entity(&mut self, entity: Entity) {
        self.storage.remove_entity(entity);
        self.entities.free(entity);
    }

    /// Inserts a component into the entity in the world.
    pub fn insert_component<T: Component>(&mut self, entity: Entity, component: T) {
        self.storage.insert_component(
            entity,
            component,
            self.last_change_tick(),
            self.read_change_tick(),
        )
    }

    /// Inserts a bundle of components into the entity in the world.
    pub fn insert_bundle<T: Bundle>(&mut self, entity: Entity, bundle: T) {
        let change_tick = self.change_tick();
        self.storage.insert_bundle(entity, bundle, change_tick)
    }

    /// Removes a component from the entity in the world.
    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> Option<T> {
        self.storage.remove_component::<T>(entity)
    }

    /// Gets a shared reference to a component from the entity in the world.
    pub fn get_component<T: Component>(&self, entity: Entity) -> Option<Ref<T>> {
        let last_change_tick = self.last_change_tick();
        let change_tick = self.read_change_tick();
        self.storage
            .get_component::<T>(entity, last_change_tick, change_tick)
    }

    /// Gets a mutable reference to a component from the entity in the world.
    pub fn get_component_mut<T: Component>(&self, entity: Entity) -> Option<Mut<T>> {
        let last_change_tick = self.last_change_tick();
        let change_tick = self.read_change_tick();
        self.storage
            .get_component_mut::<T>(entity, last_change_tick, change_tick)
    }

    /// Checks if the entity in the world has a certain type of component.
    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        self.storage.has_component::<T>(entity)
    }

    /// Queries the world for entities with components that match the query.
    pub fn query<Q: QueryFetch>(&self) -> QueryState<Q, ()> {
        QueryState::new(self)
    }

    /// Queries the world for entities with components that match the query and filter.
    pub fn query_filtered<Q: QueryFetch, F: QueryFilter>(&self) -> QueryState<Q, F> {
        QueryState::new(self)
    }

    /// Gets a shared reference to a resource from the world.
    pub fn get_resource<T: Resource>(&self) -> Option<Res<'_, T>> {
        self.resources
            .get::<T>(self.last_change_tick, self.read_change_tick())
    }

    /// Gets a mutable reference to a resource from the world.
    pub fn get_resource_mut<T: Resource>(&mut self) -> Option<ResMut<'_, T>> {
        let change_tick = self.change_tick();
        self.resources
            .get_mut::<T>(self.last_change_tick, change_tick)
    }

    /// Checks if the world has a certain type of resource.
    pub fn has_resource<T: Resource>(&self) -> bool {
        self.resources.contains::<T>()
    }

    /// Allows for multiple resources to be accessed and modified simultaneously, similarly as to in a system.
    ///
    /// # Panics
    ///
    /// - If any of the resources are accessed twice in the same call
    /// - If any of the resources have been accessed and not released prior to this call
    /// - If any of the resources do not exist
    pub fn get_many_resources_mut<'w, T>(&'w mut self) -> T::Output
    where
        T: MultiResource<'w>,
    {
        T::fetch(self)
    }

    /// Attempts to run the given system once. If the system cannot run (such as due to missing resources, for example), `None` is returned, otherwise the output of the system is returned.
    pub fn run_system<M, S, O>(&mut self, system: S) -> Option<O>
    where
        M: 'static,
        S: IntoSystem<M>,
        S::System: System<Output = O>,
    {
        let mut system = system.into_system();
        if system.can_run(self) {
            system.initialize(self);
            Some(system.run(self))
        } else {
            None
        }
    }

    /// Initializes a resource in the world. The resource is initialized using its implementation of `FromWorld`.
    /// If the resource has already been initialized, a warning is logged and the resource is not initialized again.
    pub fn init_resource<T: Resource + FromWorld>(&mut self) {
        if self.has_resource::<T>() {
            warn_once!(
                "Resource {} already initialized; not initializing it again",
                std::any::type_name::<T>(),
            );
            return;
        }
        let resource = T::from_world(self);
        self.insert_resource(resource);
    }

    /// Inserts a resource into the world.
    /// If the resource has already been inserted, a warning is logged and the resource is not inserted again.
    pub fn insert_resource<T: Resource>(&mut self, component: T) {
        if self.has_resource::<T>() {
            warn_once!(
                "Resource {} already inserted; not inserting it again",
                std::any::type_name::<T>(),
            );
            return;
        }
        let change_tick = self.change_tick();
        self.resources.insert(component, change_tick)
    }

    /// Removes a resource from the world.
    pub fn remove_resource<T: Resource>(&mut self) -> Option<T> {
        self.resources.remove::<T>().map(|(resource, _)| resource)
    }

    /// Increments the world's change tick, marking a change in the world.
    pub fn increment_change_tick(&mut self) {
        self.last_change_tick = Tick::new(self.change_tick.fetch_add(1, Ordering::AcqRel));
    }

    /// Returns the world's change tick. The change tick is acquired via an atomic load.
    pub fn read_change_tick(&self) -> Tick {
        Tick::new(self.change_tick.load(Ordering::Acquire))
    }

    /// Returns the world's change tick. The change tick is acquired immediately via mutable borrow.
    pub fn change_tick(&mut self) -> Tick {
        Tick::new(*self.change_tick.get_mut())
    }

    /// Returns the world's last change tick.
    pub fn last_change_tick(&self) -> Tick {
        self.last_change_tick
    }

    /// Pushes a system stage to the end of the "init" system schedule.
    pub fn push_init_stage<T: SystemStage>(&mut self) {
        self.systems.push_init_stage::<T>();
    }

    /// Pushes a system stage to the end of the "update" system schedule.
    pub fn push_update_stage<T: SystemStage>(&mut self) {
        self.systems.push_update_stage::<T>();
    }

    /// Pushes a system stage to the end of the "shutdown" system schedule.
    pub fn push_shutdown_stage<T: SystemStage>(&mut self) {
        self.systems.push_shutdown_stage::<T>();
    }

    /// Pushes a system stage must be run manually using [`World::run_stage`].
    pub fn push_manual_stage<T: SystemStage>(&mut self) {
        self.systems.push_manual_stage::<T>();
    }

    /// Adds an "update" system stage before another "update" system stage.
    pub fn add_update_stage_before<T: SystemStage, BEFORE: SystemStage>(&mut self) {
        self.systems.add_update_stage_before::<T, BEFORE>();
    }

    /// Adds an "update" system stage after another "update" system stage.
    pub fn add_update_stage_after<T: SystemStage, AFTER: SystemStage>(&mut self) {
        self.systems.add_update_stage_after::<T, AFTER>();
    }

    /// Adds a system to the given system stage. If the system has already been added to the stage, a warning is logged and the system is not added again.
    pub fn add_system<T, S, M>(&mut self, system: S, stage: T)
    where
        T: SystemStage,
        S: IntoSystem<M>,
        S::System: System<Output = ()>,
        M: 'static,
    {
        if self.has_system(&system, &stage) {
            warn_once!(
                "System {} already added to schedule; not adding it again",
                system.name(),
            );
            return;
        }
        self.systems.add_system(system, stage);
    }

    /// Adds a system before another system in the given system stage. If the system has already been added to the stage, a warning is logged and the system is not added again.
    pub fn add_system_before<T, M1, M2, S, BEFORE>(&mut self, system: S, before: BEFORE, stage: T)
    where
        T: SystemStage,
        M1: 'static,
        M2: 'static,
        S: IntoSystem<M1>,
        BEFORE: IntoSystem<M2>,
        S::System: System<Output = ()>,
        BEFORE::System: System<Output = ()>,
    {
        if self.has_system(&system, &stage) {
            warn_once!(
                "System {} already added to schedule; not adding it again",
                system.name(),
            );
            return;
        }
        self.systems.add_system_before(system, before, stage);
    }

    /// Adds a system after another system in the given system stage. If the system has already been added to the stage, a warning is logged and the system is not added again.
    pub fn add_system_after<T, M1, M2, S, AFTER>(&mut self, system: S, after: AFTER, stage: T)
    where
        T: SystemStage,
        M1: 'static,
        M2: 'static,
        S: IntoSystem<M1>,
        AFTER: IntoSystem<M2>,
        S::System: System<Output = ()>,
        AFTER::System: System<Output = ()>,
    {
        if self.has_system(&system, &stage) {
            warn_once!(
                "System {} already added to schedule; not adding it again",
                system.name(),
            );
            return;
        }
        self.systems.add_system_after(system, after, stage);
    }

    /// Checks if the system has been added to the given system stage.
    pub fn has_system<M: 'static>(
        &self,
        system: &impl IntoSystem<M>,
        stage: &impl SystemStage,
    ) -> bool {
        self.systems.has_system(system, stage)
    }

    /// Runs the "init" system schedule once.
    pub fn init(&mut self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_init(self)?;
        self.systems = systems;
        Ok(())
    }

    /// Runs the "update" system schedule once.
    pub fn update(&mut self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_update(self)?;
        self.systems = systems;
        Ok(())
    }

    /// Runs the "shutdown" system schedule once.
    pub fn shutdown(&mut self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_shutdown(self)?;
        self.systems = systems;
        Ok(())
    }

    /// Runs the given system stage once.
    pub fn run_stage<S: SystemStage>(&mut self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_stage::<S>(self)?;
        self.systems = systems;
        Ok(())
    }
}

/// A trait for creating a new instance of a type from a world.
pub trait FromWorld {
    fn from_world(world: &mut World) -> Self;
}

impl<T: Default> FromWorld for T {
    fn from_world(_: &mut World) -> Self {
        T::default()
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use weaver_ecs_macros::Component;
    use weaver_reflect_macros::Reflect;

    use super::*;
    use crate::{self as weaver_ecs, prelude::Query};

    #[derive(Debug, Clone, Copy, PartialEq, Reflect, Resource)]
    pub struct TestResource {
        pub value: f32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Reflect, Resource)]
    pub struct TestResource2 {
        pub value: f32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Reflect, Component)]
    pub struct Position {
        pub x: f32,
        pub y: f32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Reflect, Component)]
    pub struct Velocity {
        pub dx: f32,
        pub dy: f32,
    }

    struct Update1;
    impl SystemStage for Update1 {}

    fn position_system_shared(query: Query<(&mut Position, &Velocity)>) {
        for (_entity, (mut position, velocity)) in query.iter() {
            position.x += velocity.dx;
            position.y += velocity.dy;
        }
    }

    #[test]
    fn test_create_destroy_entity() {
        let mut world = World::new();
        let entity = world.create_entity();
        assert_eq!(entity.id(), 0);
        assert_eq!(entity.generation(), 1);

        let entity = world.create_entity();
        assert_eq!(entity.id(), 1);
        assert_eq!(entity.generation(), 1);

        let entity = world.create_entity();
        assert_eq!(entity.id(), 2);
        assert_eq!(entity.generation(), 1);

        world.destroy_entity(Entity::new(1, NonZeroU32::MIN));

        let entity = world.create_entity();
        assert_eq!(entity.id(), 1);
        assert_eq!(entity.generation(), 2);

        let entity = world.create_entity();
        assert_eq!(entity.id(), 3);
        assert_eq!(entity.generation(), 1);
    }

    #[test]
    fn test_spawn() {
        let mut world = World::new();
        let entity = world.spawn((Position { x: 1.0, y: 2.0 }, Velocity { dx: 3.0, dy: 4.0 }));
        assert_eq!(world.get_component::<Position>(entity).unwrap().x, 1.0);
        assert_eq!(world.get_component::<Position>(entity).unwrap().y, 2.0);
        assert_eq!(world.get_component::<Velocity>(entity).unwrap().dx, 3.0);
        assert_eq!(world.get_component::<Velocity>(entity).unwrap().dy, 4.0);
    }

    #[test]
    fn test_insert_remove_component() {
        let mut world = World::new();
        let entity = world.create_entity();
        world.insert_component(entity, Position { x: 1.0, y: 2.0 });
        assert_eq!(world.get_component::<Position>(entity).unwrap().x, 1.0);
        assert_eq!(world.get_component::<Position>(entity).unwrap().y, 2.0);

        let position = world.remove_component::<Position>(entity).unwrap();
        assert_eq!(position.x, 1.0);
        assert_eq!(position.y, 2.0);
        assert!(world.get_component::<Position>(entity).is_none());
    }

    #[test]
    fn test_resource() {
        let mut world = World::new();
        world.insert_resource(TestResource { value: 1.0 });
        assert_eq!(
            world
                .get_resource::<TestResource>()
                .unwrap()
                .into_inner()
                .value,
            1.0
        );
    }

    #[test]
    fn test_system() {
        let mut world = World::new();
        world.push_update_stage::<Update1>();
        world.add_system(position_system_shared, Update1);

        let entity = world.spawn((Position { x: 1.0, y: 2.0 }, Velocity { dx: 3.0, dy: 4.0 }));
        world.update().unwrap();

        assert_eq!(world.get_component::<Position>(entity).unwrap().x, 4.0);
        assert_eq!(world.get_component::<Position>(entity).unwrap().y, 6.0);
    }
}
