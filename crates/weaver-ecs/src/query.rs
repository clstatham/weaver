use std::{any::TypeId, marker::PhantomData};

use crate::prelude::{
    Archetype, ReadOnlySystemParam, SystemAccess, SystemParam, Tick, Ticks, TicksMut,
};

use super::{
    component::Component,
    entity::Entity,
    storage::{Mut, Ref},
    world::World,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryAccess {
    ReadOnly,
    ReadWrite,
}

pub trait QueryFetch: Send + Sync {
    type Item<'w>;

    fn access() -> &'static [(TypeId, QueryAccess)];
    fn fetch(world: &World, entity: Entity) -> Option<Self::Item<'_>>;
    fn iter(
        world: &World,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = (Entity, Self::Item<'_>)> + '_;
    fn entity_iter(world: &World) -> impl Iterator<Item = Entity> + '_;

    fn test_archetype(archetype: &Archetype) -> bool {
        Self::access().iter().all(|(ty, access)| match access {
            QueryAccess::ReadOnly => archetype.contains_component_by_type_id(*ty),
            QueryAccess::ReadWrite => archetype.contains_component_by_type_id(*ty),
        })
    }
}

impl<T: Component> QueryFetch for &T {
    type Item<'w> = Ref<'w, T>;

    fn access() -> &'static [(TypeId, QueryAccess)] {
        static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryAccess)>> = std::sync::OnceLock::new();
        ACCESS.get_or_init(|| vec![(TypeId::of::<T>(), QueryAccess::ReadOnly)])
    }

    fn fetch(world: &World, entity: Entity) -> Option<Ref<'_, T>> {
        let storage = world.storage();
        let archetype = storage.get_archetype(entity)?;
        if !archetype.contains_component_by_type_id(TypeId::of::<T>()) {
            return None;
        }

        let column = archetype.get_column::<T>()?;
        let index = column.sparse_index_of(entity.as_usize())?;
        let ticks = Ticks {
            added: column.dense_added_ticks[index].read_arc(),
            changed: column.dense_changed_ticks[index].read_arc(),
            last_run: world.last_change_tick(),
            this_run: world.read_change_tick(),
        };
        let data = unsafe { &*column.dense[index].get() }
            .downcast_ref()
            .unwrap();
        let item = Ref::new(data, ticks);

        Some(item)
    }

    fn iter(
        world: &World,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = (Entity, Ref<'_, T>)> + '_ {
        let storage = world.storage();
        storage
            .archetype_iter()
            .flat_map(move |archetype| {
                if !archetype.contains_component_by_type_id(TypeId::of::<T>()) {
                    return None;
                }

                archetype.get_column::<T>().map(move |column| {
                    let column = column.into_inner();
                    column
                        .sparse_iter_with_ticks()
                        .map(move |(entity, added, changed)| {
                            let ticks = Ticks {
                                added: added.read_arc(),
                                changed: changed.read_arc(),
                                last_run,
                                this_run,
                            };
                            let data = unsafe { &*column.get(entity).unwrap().get() }
                                .downcast_ref()
                                .unwrap();
                            let item = Ref::new(data, ticks);
                            let entity = Entity::from_usize(entity);
                            (entity, item)
                        })
                })
            })
            .flatten()
    }

    fn entity_iter(world: &World) -> impl Iterator<Item = Entity> + '_ {
        let storage = world.storage();
        storage
            .archetype_iter()
            .flat_map(move |archetype| {
                if !archetype.contains_component_by_type_id(TypeId::of::<T>()) {
                    return None;
                }

                archetype.get_column::<T>().map(move |column| {
                    column
                        .into_inner()
                        .sparse_iter()
                        .map(|entity| Entity::from_usize(*entity))
                })
            })
            .flatten()
    }
}

impl<T: Component> QueryFetch for &mut T {
    type Item<'w> = Mut<'w, T>;

    fn access() -> &'static [(TypeId, QueryAccess)] {
        static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryAccess)>> = std::sync::OnceLock::new();
        ACCESS.get_or_init(|| vec![(TypeId::of::<T>(), QueryAccess::ReadWrite)])
    }

    fn fetch(world: &World, entity: Entity) -> Option<Mut<'_, T>> {
        let storage = world.storage();
        let archetype = storage.get_archetype(entity)?;
        if !archetype.contains_component_by_type_id(TypeId::of::<T>()) {
            return None;
        }

        let column = archetype.get_column::<T>()?;
        let index = column.sparse_index_of(entity.as_usize())?;
        let ticks = TicksMut {
            added: column.dense_added_ticks[index].write_arc(),
            changed: column.dense_changed_ticks[index].write_arc(),
            last_run: world.last_change_tick(),
            this_run: world.read_change_tick(),
        };
        let data = unsafe { &mut *column.dense[index].get() }
            .downcast_mut()
            .unwrap();
        let item = Mut::new(data, ticks);

        Some(item)
    }

    fn iter(
        world: &World,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = (Entity, Mut<'_, T>)> + '_ {
        let storage = world.storage();
        storage
            .archetype_iter()
            .flat_map(move |archetype| {
                if !archetype.contains_component_by_type_id(TypeId::of::<T>()) {
                    return None;
                }

                archetype.get_column::<T>().map(move |column| {
                    let column = column.into_inner();
                    column
                        .sparse_iter_with_ticks()
                        .map(move |(entity, added, changed)| {
                            let ticks = TicksMut {
                                added: added.write_arc(),
                                changed: changed.write_arc(),
                                last_run,
                                this_run,
                            };
                            let data = unsafe { &mut *column.get(entity).unwrap().get() }
                                .downcast_mut()
                                .unwrap();
                            let item = Mut::new(data, ticks);
                            let entity = Entity::from_usize(entity);
                            (entity, item)
                        })
                })
            })
            .flatten()
    }

    fn entity_iter(world: &World) -> impl Iterator<Item = Entity> + '_ {
        let storage = world.storage();
        storage
            .archetype_iter()
            .flat_map(move |archetype| {
                if !archetype.contains_component_by_type_id(TypeId::of::<T>()) {
                    return None;
                }

                archetype.get_column::<T>().map(move |column| {
                    column
                        .into_inner()
                        .sparse_iter()
                        .map(|entity| Entity::from_usize(*entity))
                })
            })
            .flatten()
    }
}

pub type QueryFetchItem<'w, T> = <T as QueryFetch>::Item<'w>;

impl QueryFetch for () {
    type Item<'w> = ();
    fn access() -> &'static [(TypeId, QueryAccess)] {
        static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryAccess)>> = std::sync::OnceLock::new();
        ACCESS.get_or_init(Vec::new)
    }

    fn fetch(_: &World, _: Entity) -> Option<Self::Item<'_>> {
        Some(())
    }

    fn iter(_: &World, _: Tick, _: Tick) -> impl Iterator<Item = (Entity, Self::Item<'_>)> {
        std::iter::empty()
    }

    fn entity_iter(_world: &World) -> impl Iterator<Item = Entity> + '_ {
        // todo
        std::iter::empty()
    }
}

impl<T: QueryFetch> QueryFetch for Option<T> {
    type Item<'w> = Option<T::Item<'w>>;
    fn access() -> &'static [(TypeId, QueryAccess)] {
        T::access()
    }

    fn fetch(world: &World, entity: Entity) -> Option<Self::Item<'_>> {
        Some(T::fetch(world, entity))
    }

    fn iter(
        world: &World,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = (Entity, Option<T::Item<'_>>)> + '_ {
        T::iter(world, last_run, this_run).map(|(entity, item)| (entity, Some(item)))
    }

    fn entity_iter(world: &World) -> impl Iterator<Item = Entity> + '_ {
        T::entity_iter(world)
    }
}

macro_rules! impl_query_fetch {
    ($($param:ident),*) => {
        impl<$($param: QueryFetch),*> QueryFetch for ($($param,)*) {
            type Item<'w> = ($(
                <$param as QueryFetch>::Item<'w>,
                )*);

            fn access() -> &'static [(TypeId, QueryAccess)] {
                static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryAccess)>> = std::sync::OnceLock::new();
                ACCESS.get_or_init(|| vec![$($param::access(),)*].concat())
            }

            fn fetch(world: &World, entity: Entity) -> Option<Self::Item<'_>> {
                Some((
                    $(
                        <$param as QueryFetch>::fetch(world, entity)?,
                    )*
                ))
            }

            #[allow(non_snake_case, unused)]
            fn iter<'w>(world: &'w World, last_run: Tick, this_run: Tick) -> impl Iterator<Item = (Entity, Self::Item<'w>)> + '_ {
                itertools::multizip(($(<$param as QueryFetch>::iter(world, last_run, this_run),)*))
                .map(|params| {
                    let ($($param,)*) = params;
                    $(
                        let entity = $param.0;
                    )*
                    (
                        entity,
                        ($($param.1,)*),
                    )
                })
            }

            #[allow(non_snake_case, unused)]
            fn entity_iter(world: &World) -> impl Iterator<Item = Entity> + '_ {
                itertools::multizip(($(<$param as QueryFetch>::entity_iter(world),)*))
                .map(|params| {
                    let ($($param,)*) = params;
                    $(
                        let entity = $param;
                    )*
                    entity
                })
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

pub trait QueryFilter: Send + Sync {
    fn test_archetype(archetype: &Archetype) -> bool;
}

impl QueryFilter for () {
    fn test_archetype(_: &Archetype) -> bool {
        true
    }
}

macro_rules! impl_query_filter {
    ($($param:ident),*) => {
        impl<$($param: QueryFilter),*> QueryFilter for ($($param,)*) {
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
    fn test_archetype(archetype: &Archetype) -> bool {
        archetype.contains_component_by_type_id(TypeId::of::<T>())
    }
}

pub struct Without<T: Component>(PhantomData<T>);

impl<T: Component> QueryFilter for Without<T> {
    fn test_archetype(archetype: &Archetype) -> bool {
        !archetype.contains_component_by_type_id(TypeId::of::<T>())
    }
}

pub struct QueryState<Q, F = ()>
where
    Q: QueryFetch + ?Sized,
    F: QueryFilter + ?Sized,
{
    last_run: Tick,
    this_run: Tick,
    _fetch: PhantomData<Q>,
    _filter: PhantomData<F>,
}

impl<Q, F> QueryState<Q, F>
where
    Q: QueryFetch + ?Sized,
    F: QueryFilter + ?Sized,
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
        let storage = world.storage();
        let archetype = storage.get_archetype(entity)?;
        if !Q::test_archetype(archetype) {
            return None;
        }
        if !F::test_archetype(archetype) {
            return None;
        }

        Q::fetch(world, entity)
    }

    pub fn entity_iter<'w>(&'w self, world: &'w World) -> impl Iterator<Item = Entity> + '_ {
        Q::entity_iter(world)
    }

    pub fn iter<'w>(
        &'w self,
        world: &'w World,
    ) -> impl Iterator<Item = (Entity, Q::Item<'w>)> + '_ {
        Q::iter(world, self.last_run, self.this_run)
    }
}

pub struct Query<'w, 's, Q, F = ()>
where
    Q: QueryFetch + ?Sized + 'w,
    F: QueryFilter + ?Sized + 'w,
{
    pub(crate) state: &'s QueryState<Q, F>,
    pub(crate) world: &'w World,
}

impl<'w, 's: 'w, Q, F> Query<'w, 's, Q, F>
where
    Q: QueryFetch + ?Sized,
    F: QueryFilter + ?Sized,
{
    pub fn get(&self, entity: Entity) -> Option<Q::Item<'w>> {
        self.state.get(self.world, entity)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Entity, Q::Item<'w>)> + '_ {
        self.state.iter(self.world)
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = Entity> + '_ {
        Q::entity_iter(self.world)
    }
}

impl<Q, F> SystemParam for Query<'_, '_, Q, F>
where
    Q: QueryFetch + ?Sized + 'static,
    F: QueryFilter + ?Sized + 'static,
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
            resources_read: Vec::new(),
            resources_written: Vec::new(),
            components_read: Q::access()
                .iter()
                .filter_map(|(ty, access)| {
                    if let QueryAccess::ReadOnly = access {
                        Some(*ty)
                    } else {
                        None
                    }
                })
                .collect(),
            components_written: Q::access()
                .iter()
                .filter_map(|(ty, access)| {
                    if let QueryAccess::ReadWrite = access {
                        Some(*ty)
                    } else {
                        None
                    }
                })
                .collect(),
        }
    }

    unsafe fn fetch<'w, 's>(state: &'s mut Self::State, world: &'w World) -> Self::Item<'w, 's> {
        Query { state, world }
    }

    fn can_run(_world: &World) -> bool {
        true
    }
}

impl<Q, F> ReadOnlySystemParam for Query<'_, '_, Q, F>
where
    Q: QueryFetch + ?Sized + 'static,
    F: QueryFilter + ?Sized + 'static,
{
}
