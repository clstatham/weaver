use std::{
    cell::UnsafeCell,
    marker::PhantomData,
    ptr::NonNull,
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
};

use weaver_util::{lock::Lock, prelude::Result, warn_once};

use crate::prelude::{
    Bundle, IntoSystem, QueryFetch, QueryFilter, QueryState, Res, ResMut, Resource, Resources,
    SystemSchedule, SystemStage, Tick,
};

use super::{
    component::Component,
    entity::Entity,
    storage::{Mut, Ref, Storage},
};

#[derive(Clone, Copy)]
pub struct UnsafeWorldCell<'a>(
    NonNull<World>,
    PhantomData<&'a mut World>,
    PhantomData<&'a UnsafeCell<World>>,
);
unsafe impl Sync for UnsafeWorldCell<'_> {}

impl<'a> UnsafeWorldCell<'a> {
    pub fn new_exclusive(world: &'a mut World) -> Self {
        unsafe {
            Self(
                NonNull::new_unchecked(world as *mut _),
                PhantomData,
                PhantomData,
            )
        }
    }

    pub fn new_shared(world: &'a World) -> Self {
        unsafe {
            Self(
                NonNull::new_unchecked(world as *const _ as *mut _),
                PhantomData,
                PhantomData,
            )
        }
    }

    /// # Safety
    ///
    /// This function is unsafe because it dereferences the pointer to the world.
    /// Callers must ensure that the world is not accessed concurrently, and that no mutable references to the world are held.
    pub unsafe fn world(self) -> &'a World {
        unsafe { self.0.as_ref() }
    }

    /// # Safety
    ///
    /// This function is unsafe because it dereferences the pointer to the world.
    /// Callers must ensure that the world is not accessed concurrently, and that no other references to the world are held.
    pub unsafe fn world_mut(mut self) -> &'a mut World {
        unsafe { self.0.as_mut() }
    }

    /// # Safety
    ///
    /// This function is unsafe because it dereferences the pointer to the world.
    pub unsafe fn get_resource<T: Resource>(self) -> Option<Res<'a, T>> {
        unsafe { self.0.as_ref().get_resource::<T>() }
    }

    /// # Safety
    ///
    /// This function is unsafe because it dereferences the pointer to the world.
    pub unsafe fn get_resource_mut<T: Resource>(mut self) -> Option<ResMut<'a, T>> {
        unsafe { self.0.as_mut().get_resource_mut::<T>() }
    }

    pub fn read_change_tick(self) -> Tick {
        unsafe { self.0.as_ref().read_change_tick() }
    }

    pub fn last_change_tick(self) -> Tick {
        unsafe { self.0.as_ref().last_change_tick() }
    }
}

pub struct World {
    next_entity: AtomicU32,
    free_entities: Lock<Vec<Entity>>,
    storage: Storage,
    resources: Resources,
    change_tick: AtomicU64,
    last_change_tick: Tick,
    systems: SystemSchedule,
}

unsafe impl Send for World {}
unsafe impl Sync for World {}

impl Default for World {
    fn default() -> Self {
        Self {
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

    pub fn as_unsafe_world_cell(&self) -> UnsafeWorldCell {
        UnsafeWorldCell::new_shared(self)
    }

    pub fn as_unsafe_world_cell_exclusive(&mut self) -> UnsafeWorldCell {
        UnsafeWorldCell::new_exclusive(self)
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    pub fn storage_mut(&mut self) -> &mut Storage {
        &mut self.storage
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
        log::trace!(
            "Inserting component {:?} for entity {:?}",
            std::any::type_name::<T>(),
            entity
        );
        self.storage.insert_component(
            entity,
            component,
            self.last_change_tick(),
            self.read_change_tick(),
        )
    }

    pub fn insert_components<T: Bundle>(&mut self, entity: Entity, bundle: T) {
        self.storage
            .insert_components(entity, bundle, self.read_change_tick())
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> Option<T> {
        log::trace!(
            "Removing component {:?} for entity {:?}",
            std::any::type_name::<T>(),
            entity
        );
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

    pub fn query<Q: QueryFetch>(&self) -> QueryState<Q, ()> {
        QueryState::new(self)
    }

    pub fn query_filtered<Q: QueryFetch, F: QueryFilter>(&self) -> QueryState<Q, F> {
        QueryState::new(self)
    }

    pub fn get_resource<T: Resource>(&self) -> Option<Res<'_, T>> {
        self.resources.get::<T>()
    }

    pub fn get_resource_mut<T: Resource>(&mut self) -> Option<ResMut<'_, T>> {
        self.resources
            .get_mut::<T>(self.last_change_tick, self.read_change_tick())
    }

    pub fn has_resource<T: Resource>(&self) -> bool {
        self.resources.contains::<T>()
    }

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

    pub fn add_system<S: SystemStage, M: 'static>(&mut self, system: impl IntoSystem<M>, stage: S) {
        if self.has_system(&system, &stage) {
            warn_once!(
                "System {} already added to schedule; not adding it again",
                system.name(),
            );
            return;
        }
        self.systems.add_system(system, stage);
    }

    pub fn add_system_before<S: SystemStage, M1: 'static, M2: 'static>(
        &mut self,
        system: impl IntoSystem<M1>,
        before: impl IntoSystem<M2>,
        stage: S,
    ) {
        if self.has_system(&system, &stage) {
            warn_once!(
                "System {} already added to schedule; not adding it again",
                system.name(),
            );
            return;
        }
        self.systems.add_system_before(system, before, stage);
    }

    pub fn add_system_after<S: SystemStage, M1: 'static, M2: 'static>(
        &mut self,
        system: impl IntoSystem<M1>,
        after: impl IntoSystem<M2>,
        stage: S,
    ) {
        if self.has_system(&system, &stage) {
            warn_once!(
                "System {} already added to schedule; not adding it again",
                system.name(),
            );
            return;
        }
        self.systems.add_system_after(system, after, stage);
    }

    pub fn has_system<M: 'static>(
        &self,
        system: &impl IntoSystem<M>,
        stage: &impl SystemStage,
    ) -> bool {
        self.systems.has_system(system, stage)
    }

    pub fn init(&mut self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_init(self)?;
        self.systems = systems;
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_update(self)?;
        self.systems = systems;
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_shutdown(self)?;
        self.systems = systems;
        Ok(())
    }

    pub fn run_stage<S: SystemStage>(&mut self) -> Result<()> {
        let mut systems = std::mem::take(&mut self.systems);
        systems.run_stage::<S>(self)?;
        self.systems = systems;
        Ok(())
    }
}

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
    use weaver_ecs_macros::Component;
    use weaver_reflect_macros::Reflect;

    use super::*;
    use crate::{self as weaver_ecs, prelude::Query};

    #[derive(Debug, Clone, Copy, PartialEq, Reflect, Resource)]
    pub struct TestResource {
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

    fn position_system_shared(query: Query<(&mut Position, &Velocity)>) -> Result<()> {
        for (_entity, (mut position, velocity)) in query.iter() {
            position.x += velocity.dx;
            position.y += velocity.dy;
        }
        Ok(())
    }

    #[test]
    fn test_create_destroy_entity() {
        let mut world = World::new();
        let entity = world.create_entity();
        assert_eq!(entity.id(), 0);
        assert_eq!(entity.generation(), 0);

        let entity = world.create_entity();
        assert_eq!(entity.id(), 1);
        assert_eq!(entity.generation(), 0);

        let entity = world.create_entity();
        assert_eq!(entity.id(), 2);
        assert_eq!(entity.generation(), 0);

        world.destroy_entity(Entity::new(1, 0));

        let entity = world.create_entity();
        assert_eq!(entity.id(), 1);
        assert_eq!(entity.generation(), 1);

        let entity = world.create_entity();
        assert_eq!(entity.id(), 3);
        assert_eq!(entity.generation(), 0);
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
        assert_eq!(world.get_resource::<TestResource>().unwrap().value, 1.0);
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
