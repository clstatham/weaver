use std::{any::TypeId, marker::PhantomData};

use weaver_util::FxHashSet;

use crate::prelude::{
    Archetype, SystemAccess, SystemParam, Tick, Ticks, TicksMut, UnsafeWorldCell,
};

use super::{
    component::Component,
    entity::Entity,
    storage::{Mut, Ref},
    world::World,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryFetchAccess {
    ReadOnly,
    ReadWrite,
}

pub trait QueryFetch: Send + Sync {
    type Item<'w>;

    fn access() -> &'static [(TypeId, QueryFetchAccess)];

    fn fetch<F: QueryFilter>(world: &World, entity: Entity) -> Option<Self::Item<'_>>;

    fn entity_iter<F: QueryFilter>(world: &World) -> impl Iterator<Item = Entity> + '_;

    fn iter<F: QueryFilter>(
        world: &World,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = (Entity, Self::Item<'_>)> + '_;

    fn test_archetype(archetype: &Archetype) -> bool;
}

impl<T: Component> QueryFetch for &T {
    type Item<'w> = Ref<'w, T>;

    fn access() -> &'static [(TypeId, QueryFetchAccess)] {
        static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryFetchAccess)>> =
            std::sync::OnceLock::new();
        ACCESS.get_or_init(|| vec![(TypeId::of::<T>(), QueryFetchAccess::ReadOnly)])
    }

    fn fetch<F: QueryFilter>(world: &World, entity: Entity) -> Option<Ref<'_, T>> {
        let storage = world.storage();
        let archetype = storage.get_archetype(entity)?;
        if !Self::test_archetype(archetype) {
            return None;
        }
        if !F::test_archetype(archetype) {
            return None;
        }

        let column = archetype.get_column::<T>()?.into_inner();
        let index = column.dense_index_of(entity.id())?;
        let ticks = Ticks {
            added: column.dense_added_ticks[index].read(),
            changed: column.dense_changed_ticks[index].read(),
            last_run: world.last_change_tick(),
            this_run: world.read_change_tick(),
        };
        let data = unsafe { &*column.dense[index].get() }
            .downcast_ref()
            .unwrap();
        let item = Ref::new(data, ticks);

        Some(item)
    }

    fn entity_iter<F: QueryFilter>(world: &World) -> impl Iterator<Item = Entity> + '_ {
        let storage = world.storage();
        storage
            .archetype_iter()
            .filter(move |archetype| {
                Self::test_archetype(archetype) && F::test_archetype(archetype)
            })
            .flat_map(move |archetype| {
                archetype.get_column::<T>().map(move |column| {
                    column
                        .into_inner()
                        .sparse_iter()
                        .map(|entity| world.find_entity_by_id(entity).unwrap())
                })
            })
            .flatten()
    }

    fn iter<F: QueryFilter>(
        world: &World,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = (Entity, Ref<'_, T>)> + '_ {
        let storage = world.storage();
        storage
            .archetype_iter()
            .filter(move |archetype| {
                Self::test_archetype(archetype) && F::test_archetype(archetype)
            })
            .flat_map(move |archetype| {
                archetype.get_column::<T>().map(move |column| {
                    let column = column.into_inner();
                    column
                        .sparse_iter_with_ticks()
                        .map(move |(entity, added, changed)| {
                            let ticks = Ticks {
                                added,
                                changed,
                                last_run,
                                this_run,
                            };
                            let data = unsafe { &*column.get(entity).unwrap().get() }
                                .downcast_ref()
                                .unwrap();
                            let item = Ref::new(data, ticks);
                            let entity = world.find_entity_by_id(entity).unwrap();
                            (entity, item)
                        })
                })
            })
            .flatten()
    }

    fn test_archetype(archetype: &Archetype) -> bool {
        archetype.contains_component_by_type_id(TypeId::of::<T>())
    }
}

impl<T: Component> QueryFetch for &mut T {
    type Item<'w> = Mut<'w, T>;

    fn access() -> &'static [(TypeId, QueryFetchAccess)] {
        static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryFetchAccess)>> =
            std::sync::OnceLock::new();
        ACCESS.get_or_init(|| vec![(TypeId::of::<T>(), QueryFetchAccess::ReadWrite)])
    }

    fn fetch<F: QueryFilter>(world: &World, entity: Entity) -> Option<Mut<'_, T>> {
        let storage = world.storage();
        let archetype = storage.get_archetype(entity)?;
        if !Self::test_archetype(archetype) {
            return None;
        }
        if !F::test_archetype(archetype) {
            return None;
        }

        let column = archetype.get_column::<T>()?.into_inner();
        let index = column.dense_index_of(entity.id())?;
        let ticks = TicksMut {
            added: column.dense_added_ticks[index].write(),
            changed: column.dense_changed_ticks[index].write(),
            last_run: world.last_change_tick(),
            this_run: world.read_change_tick(),
        };
        let data = unsafe { &mut *column.dense[index].get() }
            .downcast_mut()
            .unwrap();
        let item = Mut::new(data, ticks);

        Some(item)
    }

    fn iter<F: QueryFilter>(
        world: &World,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = (Entity, Mut<'_, T>)> + '_ {
        let storage = world.storage();
        storage
            .archetype_iter()
            .filter(move |archetype| {
                Self::test_archetype(archetype) && F::test_archetype(archetype)
            })
            .flat_map(move |archetype| {
                archetype.get_column::<T>().map(move |column| {
                    let column = column.into_inner();
                    column
                        .sparse_iter_with_ticks_mut()
                        .map(move |(entity, added, changed)| {
                            let ticks = TicksMut {
                                added,
                                changed,
                                last_run,
                                this_run,
                            };
                            let data = unsafe { &mut *column.get(entity).unwrap().get() }
                                .downcast_mut()
                                .unwrap();
                            let item = Mut::new(data, ticks);
                            let entity = world.find_entity_by_id(entity).unwrap();
                            (entity, item)
                        })
                })
            })
            .flatten()
    }

    fn entity_iter<F: QueryFilter>(world: &World) -> impl Iterator<Item = Entity> + '_ {
        let storage = world.storage();
        storage
            .archetype_iter()
            .filter(move |archetype| {
                Self::test_archetype(archetype) && F::test_archetype(archetype)
            })
            .flat_map(move |archetype| {
                archetype.get_column::<T>().map(move |column| {
                    column
                        .into_inner()
                        .sparse_iter()
                        .map(|entity| world.find_entity_by_id(entity).unwrap())
                })
            })
            .flatten()
    }

    fn test_archetype(archetype: &Archetype) -> bool {
        archetype.contains_component_by_type_id(TypeId::of::<T>())
    }
}

pub type QueryFetchItem<'w, T> = <T as QueryFetch>::Item<'w>;

impl QueryFetch for () {
    type Item<'w> = ();
    fn access() -> &'static [(TypeId, QueryFetchAccess)] {
        static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryFetchAccess)>> =
            std::sync::OnceLock::new();
        ACCESS.get_or_init(Vec::new)
    }

    fn fetch<F: QueryFilter>(world: &World, entity: Entity) -> Option<Self::Item<'_>> {
        let storage = world.storage();
        let archetype = storage.get_archetype(entity)?;
        if !F::test_archetype(archetype) {
            return None;
        }
        Some(())
    }

    fn iter<F: QueryFilter>(
        world: &World,
        _: Tick,
        _: Tick,
    ) -> impl Iterator<Item = (Entity, Self::Item<'_>)> + '_ {
        world
            .storage()
            .archetype_iter()
            .filter(move |archetype| F::test_archetype(archetype))
            .flat_map(|archetype| archetype.entity_iter().map(|entity| (entity, ())))
    }

    fn entity_iter<F: QueryFilter>(world: &World) -> impl Iterator<Item = Entity> + '_ {
        world
            .storage()
            .archetype_iter()
            .filter(move |archetype| F::test_archetype(archetype))
            .flat_map(|archetype| archetype.entity_iter())
    }

    fn test_archetype(_archetype: &Archetype) -> bool {
        true
    }
}

impl<T: QueryFetch> QueryFetch for Option<T> {
    type Item<'w> = Option<T::Item<'w>>;
    fn access() -> &'static [(TypeId, QueryFetchAccess)] {
        T::access()
    }

    fn fetch<F: QueryFilter>(world: &World, entity: Entity) -> Option<Self::Item<'_>> {
        if let Some(item) = T::fetch::<F>(world, entity) {
            Some(Some(item))
        } else {
            Some(None)
        }
    }

    fn iter<F: QueryFilter>(
        world: &World,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = (Entity, Option<T::Item<'_>>)> + '_ {
        T::iter::<F>(world, last_run, this_run).map(|(entity, item)| (entity, Some(item)))
    }

    fn entity_iter<F: QueryFilter>(world: &World) -> impl Iterator<Item = Entity> + '_ {
        T::entity_iter::<F>(world)
    }

    fn test_archetype(_archetype: &Archetype) -> bool {
        true
    }
}

macro_rules! impl_query_fetch {
    ($($param:ident),*) => {
        impl<$($param: QueryFetch),*> QueryFetch for ($($param,)*) {
            type Item<'w> = ($(
                <$param as QueryFetch>::Item<'w>,
                )*);

            fn access() -> &'static [(TypeId, QueryFetchAccess)] {
                static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryFetchAccess)>> = std::sync::OnceLock::new();
                ACCESS.get_or_init(|| vec![$($param::access(),)*].concat())
            }

            fn fetch<Filter: QueryFilter>(world: &World, entity: Entity) -> Option<Self::Item<'_>> {
                Some((
                    $(
                        <$param as QueryFetch>::fetch::<Filter>(world, entity)?,
                    )*
                ))
            }

            #[allow(non_snake_case, unused)]
            fn iter<'w, Filter: QueryFilter>(world: &'w World, last_run: Tick, this_run: Tick) -> impl Iterator<Item = (Entity, Self::Item<'w>)> + '_ {
                let storage = world.storage();
                storage
                    .archetype_iter()
                    .filter(move |archetype| Self::test_archetype(archetype) && Filter::test_archetype(archetype))
                    .flat_map(move |archetype| {
                        archetype.entity_iter().filter_map(move |entity| {
                            Self::fetch::<Filter>(world, entity).map(|item| (entity, item))
                        })
                    })
            }

            #[allow(non_snake_case, unused)]
            fn entity_iter<Filter: QueryFilter>(world: &World) -> impl Iterator<Item = Entity> + '_ {
                let storage = world.storage();
                storage
                    .archetype_iter()
                    .filter(move |archetype| Self::test_archetype(archetype) && Filter::test_archetype(archetype))
                    .flat_map(move |archetype| archetype.entity_iter())
            }

            fn test_archetype(archetype: &Archetype) -> bool {
                $(
                    $param::test_archetype(archetype) &&
                )*
                true
            }
        }
    };
}

impl_query_fetch!(A);
impl_query_fetch!(A, B);
impl_query_fetch!(A, B, C);
impl_query_fetch!(A, B, C, D);
impl_query_fetch!(A, B, C, D, E);
impl_query_fetch!(A, B, C, D, E, F);
impl_query_fetch!(A, B, C, D, E, F, G);
impl_query_fetch!(A, B, C, D, E, F, G, H);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryFilterAccess {
    With,
    Without,
}

pub trait QueryFilter: Send + Sync {
    fn access() -> &'static [(TypeId, QueryFilterAccess)];
    fn test_archetype(archetype: &Archetype) -> bool;
}

impl QueryFilter for () {
    fn access() -> &'static [(TypeId, QueryFilterAccess)] {
        static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryFilterAccess)>> =
            std::sync::OnceLock::new();
        ACCESS.get_or_init(Vec::new)
    }

    fn test_archetype(_: &Archetype) -> bool {
        true
    }
}

macro_rules! impl_query_filter {
    ($($param:ident),*) => {
        impl<$($param: QueryFilter),*> QueryFilter for ($($param,)*) {
            fn access() -> &'static [(TypeId, QueryFilterAccess)] {
                static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryFilterAccess)>> = std::sync::OnceLock::new();
                ACCESS.get_or_init(|| vec![$($param::access(),)*].concat())
            }

            fn test_archetype(archetype: &Archetype) -> bool {
                $(
                    $param::test_archetype(archetype) &&
                )*
                true
            }
        }
    };
}

impl_query_filter!(A);
impl_query_filter!(A, B);
impl_query_filter!(A, B, C);
impl_query_filter!(A, B, C, D);
impl_query_filter!(A, B, C, D, E);
impl_query_filter!(A, B, C, D, E, F);
impl_query_filter!(A, B, C, D, E, F, G);
impl_query_filter!(A, B, C, D, E, F, G, H);

pub struct With<T: Component>(PhantomData<T>);

impl<T: Component> QueryFilter for With<T> {
    fn access() -> &'static [(TypeId, QueryFilterAccess)] {
        static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryFilterAccess)>> =
            std::sync::OnceLock::new();
        ACCESS.get_or_init(|| vec![(TypeId::of::<T>(), QueryFilterAccess::With)])
    }

    fn test_archetype(archetype: &Archetype) -> bool {
        archetype.contains_component_by_type_id(TypeId::of::<T>())
    }
}

pub struct Without<T: Component>(PhantomData<T>);

impl<T: Component> QueryFilter for Without<T> {
    fn access() -> &'static [(TypeId, QueryFilterAccess)] {
        static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryFilterAccess)>> =
            std::sync::OnceLock::new();
        ACCESS.get_or_init(|| vec![(TypeId::of::<T>(), QueryFilterAccess::Without)])
    }

    fn test_archetype(archetype: &Archetype) -> bool {
        !archetype.contains_component_by_type_id(TypeId::of::<T>())
    }
}

pub struct QueryState<Q, F = ()>
where
    Q: QueryFetch,
    F: QueryFilter,
{
    last_run: Tick,
    this_run: Tick,
    _fetch: PhantomData<Q>,
    _filter: PhantomData<F>,
}

impl<Q, F> QueryState<Q, F>
where
    Q: QueryFetch,
    F: QueryFilter,
{
    pub fn new(world: &World) -> Self {
        Self {
            last_run: world.last_change_tick(),
            this_run: world.read_change_tick(),
            _fetch: PhantomData,
            _filter: PhantomData,
        }
    }

    pub fn get<'w>(&self, world: &'w World, entity: Entity) -> Option<Q::Item<'w>> {
        Q::fetch::<F>(world, entity)
    }

    pub fn entity_iter<'w>(&'w self, world: &'w World) -> impl Iterator<Item = Entity> + '_ {
        Q::entity_iter::<F>(world)
    }

    pub fn iter<'w>(
        &'w self,
        world: &'w World,
    ) -> impl Iterator<Item = (Entity, Q::Item<'w>)> + '_ {
        Q::iter::<F>(world, self.last_run, self.this_run)
    }
}

pub struct Query<'w, 's, Q, F = ()>
where
    Q: QueryFetch + 'w,
    F: QueryFilter + 'w,
{
    pub(crate) state: &'s QueryState<Q, F>,
    pub(crate) world: UnsafeWorldCell<'w>,
}

unsafe impl<Q: QueryFetch, F: QueryFilter> Send for Query<'_, '_, Q, F> {}
unsafe impl<Q: QueryFetch, F: QueryFilter> Sync for Query<'_, '_, Q, F> {}

impl<'w, 's: 'w, Q, F> Query<'w, 's, Q, F>
where
    Q: QueryFetch,
    F: QueryFilter,
{
    pub fn get(&self, entity: Entity) -> Option<Q::Item<'w>> {
        unsafe { self.state.get(self.world.world(), entity) }
    }

    pub fn iter(&self) -> impl Iterator<Item = (Entity, Q::Item<'w>)> + '_ {
        unsafe { self.state.iter(self.world.world()) }
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = Entity> + '_ {
        unsafe { Q::entity_iter::<F>(self.world.world()) }
    }
}

unsafe impl<Q, F> SystemParam for Query<'_, '_, Q, F>
where
    Q: QueryFetch + 'static,
    F: QueryFilter + 'static,
{
    type State = QueryState<Q, F>;
    type Item<'w, 's> = Query<'w, 's, Q, F>;

    fn validate_access(access: &SystemAccess) -> bool {
        let my_access = Self::access();
        if access
            .components_read
            .iter()
            .any(|ty| my_access.components_written.contains(ty))
            || access
                .components_written
                .iter()
                .any(|ty| my_access.components_read.contains(ty))
        {
            return false;
        }
        true
    }

    fn init_state(world: &mut World) -> Self::State {
        QueryState::new(world)
    }

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: FxHashSet::default(),
            resources_written: FxHashSet::default(),
            components_read: Q::access()
                .iter()
                .filter_map(|(ty, access)| {
                    if let QueryFetchAccess::ReadOnly = access {
                        Some(*ty)
                    } else {
                        None
                    }
                })
                .collect(),
            components_written: Q::access()
                .iter()
                .filter_map(|(ty, access)| {
                    if let QueryFetchAccess::ReadWrite = access {
                        Some(*ty)
                    } else {
                        None
                    }
                })
                .collect(),
        }
    }

    unsafe fn fetch<'w, 's>(
        state: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        Query { state, world }
    }

    fn can_run(_world: &World) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::{self as weaver_ecs, prelude::Entity};
    use weaver_ecs_macros::Component;
    use weaver_reflect_macros::Reflect;

    use crate::prelude::World;

    #[derive(Debug, Clone, Copy, PartialEq, Reflect, Component)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Reflect, Component)]
    struct Velocity {
        x: f32,
        y: f32,
    }

    #[test]
    fn query_one_component() {
        let mut world = World::new();
        let entity1 = world.spawn((Position { x: 1.0, y: 2.0 }, Velocity { x: 3.0, y: 4.0 }));
        let entity2 = world.spawn((Position { x: 5.0, y: 6.0 }, Velocity { x: 7.0, y: 8.0 }));
        let entity3 = world.spawn(Position { x: 9.0, y: 10.0 });

        let query = world.query::<&Position>();
        let mut iter = query.iter(&world);

        let test_entity = |entity: Entity, position: Position| match entity {
            e if e == entity1 => {
                assert_eq!(position.x, 1.0);
                assert_eq!(position.y, 2.0);
            }
            e if e == entity2 => {
                assert_eq!(position.x, 5.0);
                assert_eq!(position.y, 6.0);
            }
            e if e == entity3 => {
                assert_eq!(position.x, 9.0);
                assert_eq!(position.y, 10.0);
            }
            _ => panic!("unexpected entity"),
        };

        let (entity, position) = iter.next().unwrap();
        test_entity(entity, *position);

        let (entity, position) = iter.next().unwrap();
        test_entity(entity, *position);

        let (entity, position) = iter.next().unwrap();
        test_entity(entity, *position);

        assert!(iter.next().is_none());
    }

    #[test]
    fn query_two_components() {
        let mut world = World::new();
        let entity1 = world.spawn((Position { x: 1.0, y: 2.0 }, Velocity { x: 3.0, y: 4.0 }));
        let entity2 = world.spawn((Position { x: 5.0, y: 6.0 }, Velocity { x: 7.0, y: 8.0 }));
        let _entity3 = world.spawn(Position { x: 9.0, y: 10.0 });

        let query = world.query::<(&Position, &Velocity)>();
        let mut iter = query.iter(&world);

        let test_entity = |entity: Entity, position: Position, velocity: Velocity| match entity {
            e if e == entity1 => {
                assert_eq!(position.x, 1.0);
                assert_eq!(position.y, 2.0);
                assert_eq!(velocity.x, 3.0);
                assert_eq!(velocity.y, 4.0);
            }
            e if e == entity2 => {
                assert_eq!(position.x, 5.0);
                assert_eq!(position.y, 6.0);
                assert_eq!(velocity.x, 7.0);
                assert_eq!(velocity.y, 8.0);
            }
            _ => panic!("unexpected entity"),
        };

        let (entity, (position, velocity)) = iter.next().unwrap();
        test_entity(entity, *position, *velocity);

        let (entity, (position, velocity)) = iter.next().unwrap();
        test_entity(entity, *position, *velocity);

        assert!(iter.next().is_none());
    }
}
