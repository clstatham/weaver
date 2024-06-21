use std::{any::TypeId, marker::PhantomData};

use itertools::Itertools;
use weaver_util::lock::SharedLock;

use crate::prelude::{
    Archetype, ChangeDetection, ColumnRef, Data, SparseSet, Storage, Tick, Ticks, TicksMut,
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

pub struct ColumnIter<T: Component> {
    column: SharedLock<SparseSet<Data>>,
    index: usize,
    last_run: Tick,
    this_run: Tick,
    _marker: PhantomData<T>,
}

impl<T: Component> Iterator for ColumnIter<T> {
    type Item = (Entity, Ref<T>);

    fn next(&mut self) -> Option<Self::Item> {
        let column = self.column.read_arc();
        if self.index == column.len() {
            return None;
        }

        let entity = column.sparse_index_of(self.index)?;
        let entity = Entity::from_usize(entity);
        let ticks = Ticks {
            added: column.dense_added_ticks[self.index].read_arc(),
            changed: column.dense_changed_ticks[self.index].read_arc(),
            last_run: self.last_run,
            this_run: self.this_run,
        };
        let item = Ref::new(self.index, column, ticks);

        self.index += 1;

        Some((entity, item))
    }
}

pub struct ColumnMutIter<T: Component> {
    column: SharedLock<SparseSet<Data>>,
    index: usize,
    last_run: Tick,
    this_run: Tick,
    _marker: PhantomData<T>,
}

impl<T: Component> Iterator for ColumnMutIter<T> {
    type Item = (Entity, Mut<T>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.column.read().len() {
            return None;
        }

        let column = self.column.write_arc();
        let entity = column.sparse_index_of(self.index)?;
        let entity = Entity::from_usize(entity);
        let ticks = TicksMut {
            added: column.dense_added_ticks[self.index].write_arc(),
            changed: column.dense_changed_ticks[self.index].write_arc(),
            last_run: self.last_run,
            this_run: self.this_run,
        };
        let item = Mut::new(self.index, column, ticks);
        self.index += 1;

        Some((entity, item))
    }
}

pub trait ColumnExt<T: Component> {
    fn dense_iter(&self, last_run: Tick, this_run: Tick) -> ColumnIter<T>;
    fn dense_iter_mut(&self, last_run: Tick, this_run: Tick) -> ColumnMutIter<T>;
}

impl<T: Component> ColumnExt<T> for ColumnRef {
    fn dense_iter(&self, last_run: Tick, this_run: Tick) -> ColumnIter<T> {
        ColumnIter {
            column: (**self).clone(),
            index: 0,
            last_run,
            this_run,
            _marker: PhantomData,
        }
    }

    fn dense_iter_mut(&self, last_run: Tick, this_run: Tick) -> ColumnMutIter<T> {
        ColumnMutIter {
            column: (**self).clone(),
            index: 0,
            last_run,
            this_run,
            _marker: PhantomData,
        }
    }
}

pub trait QueryFetchParam {
    type Item: Component;
    type Column: ColumnExt<Self::Item>;
    type Fetch;

    fn type_id() -> TypeId;
    fn access() -> QueryAccess;
    fn fetch_columns<F>(storage: &Storage, test_archetype: F) -> Vec<ColumnRef>
    where
        F: Fn(&Archetype) -> bool;
    fn iter(
        columns: &Self::Column,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = (Entity, Self::Fetch)>;
}

impl<T: Component> QueryFetchParam for &T {
    type Item = T;
    type Column = ColumnRef;
    type Fetch = Ref<T>;

    fn type_id() -> TypeId {
        TypeId::of::<T>()
    }

    fn access() -> QueryAccess {
        QueryAccess::ReadOnly
    }

    fn fetch_columns<'a, F>(storage: &Storage, test_archetype: F) -> Vec<ColumnRef>
    where
        F: Fn(&Archetype) -> bool,
    {
        let mut columns = Vec::new();
        for archetype in storage.archetype_iter() {
            if test_archetype(archetype) {
                if let Some(column) = archetype.get_column::<T>() {
                    columns.push(column);
                }
            }
        }

        columns
    }

    fn iter(
        columns: &Self::Column,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = (Entity, Self::Fetch)> {
        columns.dense_iter(last_run, this_run)
    }
}

impl<T: Component> QueryFetchParam for &mut T {
    type Item = T;
    type Column = ColumnRef;
    type Fetch = Mut<T>;

    fn type_id() -> TypeId {
        TypeId::of::<T>()
    }

    fn access() -> QueryAccess {
        QueryAccess::ReadWrite
    }

    fn fetch_columns<'a, F>(storage: &Storage, test_archetype: F) -> Vec<ColumnRef>
    where
        F: Fn(&Archetype) -> bool,
    {
        let mut columns = Vec::new();
        for archetype in storage.archetype_iter() {
            if test_archetype(archetype) {
                if let Some(column) = archetype.get_column::<T>() {
                    columns.push(column);
                }
            }
        }

        columns
    }

    fn iter(
        columns: &Self::Column,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = (Entity, Self::Fetch)> {
        columns.dense_iter_mut(last_run, this_run)
    }
}

pub trait QueryFetch: Send + Sync {
    type Columns: Send + Sync;
    type Fetch;
    fn access() -> &'static [(TypeId, QueryAccess)];
    fn fetch_columns<F>(storage: &Storage, test_archetype: &F) -> Vec<Self::Columns>
    where
        F: Fn(&Archetype) -> bool;
    fn iter(
        columns: &Self::Columns,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = (Entity, Self::Fetch)>;
    fn test_archetype(archetype: &Archetype) -> bool;
    fn any_changed(fetch: &Self::Fetch) -> bool;
}

impl QueryFetch for () {
    type Columns = ();
    type Fetch = ();
    fn access() -> &'static [(TypeId, QueryAccess)] {
        static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryAccess)>> = std::sync::OnceLock::new();
        ACCESS.get_or_init(Vec::new)
    }

    fn fetch_columns<F>(_: &Storage, _: &F) -> Vec<Self::Columns>
    where
        F: Fn(&Archetype) -> bool,
    {
        Vec::new()
    }

    fn iter(_: &Self::Columns, _: Tick, _: Tick) -> impl Iterator<Item = (Entity, Self::Fetch)> {
        std::iter::empty()
    }

    fn test_archetype(_: &Archetype) -> bool {
        true
    }

    fn any_changed(_: &Self::Fetch) -> bool {
        false
    }
}

impl<T: Component> QueryFetch for &T {
    type Columns = ColumnRef;
    type Fetch = Ref<T>;
    fn access() -> &'static [(TypeId, QueryAccess)] {
        static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryAccess)>> = std::sync::OnceLock::new();
        ACCESS.get_or_init(|| {
            vec![(
                <&T as QueryFetchParam>::type_id(),
                <&T as QueryFetchParam>::access(),
            )]
        })
    }

    fn fetch_columns<F>(storage: &Storage, test_archetype: &F) -> Vec<Self::Columns>
    where
        F: Fn(&Archetype) -> bool,
    {
        <&T as QueryFetchParam>::fetch_columns(storage, test_archetype)
    }

    fn iter(
        columns: &Self::Columns,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = (Entity, Self::Fetch)> {
        columns.dense_iter(last_run, this_run)
    }

    fn test_archetype(archetype: &Archetype) -> bool {
        archetype.contains_component_by_type_id(TypeId::of::<T>())
    }

    fn any_changed(fetch: &Self::Fetch) -> bool {
        fetch.is_changed() || fetch.is_added()
    }
}

impl<T: Component> QueryFetch for &mut T {
    type Columns = ColumnRef;
    type Fetch = Mut<T>;
    fn access() -> &'static [(TypeId, QueryAccess)] {
        static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryAccess)>> = std::sync::OnceLock::new();
        ACCESS.get_or_init(|| {
            vec![(
                <&mut T as QueryFetchParam>::type_id(),
                <&mut T as QueryFetchParam>::access(),
            )]
        })
    }

    fn fetch_columns<F>(storage: &Storage, test_archetype: &F) -> Vec<Self::Columns>
    where
        F: Fn(&Archetype) -> bool,
    {
        <&T as QueryFetchParam>::fetch_columns(storage, test_archetype)
    }

    fn iter(
        columns: &Self::Columns,
        last_run: Tick,
        this_run: Tick,
    ) -> impl Iterator<Item = (Entity, Self::Fetch)> {
        columns.dense_iter_mut(last_run, this_run)
    }

    fn test_archetype(archetype: &Archetype) -> bool {
        archetype.contains_component_by_type_id(TypeId::of::<T>())
    }

    fn any_changed(fetch: &Self::Fetch) -> bool {
        fetch.is_changed() || fetch.is_added()
    }
}

macro_rules! impl_query_fetch {
    ($($param:ident),*) => {
        impl<$($param: QueryFetch),*> QueryFetch for ($($param,)*) {
            type Columns = ($($param::Columns,)*);
            type Fetch = ($(
                <$param as QueryFetch>::Fetch,
                )*);

            fn access() -> &'static [(TypeId, QueryAccess)] {
                static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryAccess)>> = std::sync::OnceLock::new();
                ACCESS.get_or_init(|| vec![$($param::access(),)*].concat())
            }

            fn fetch_columns<Test>(storage: &Storage, test_archetype: &Test) -> Vec<Self::Columns>
            where
                Test: Fn(&Archetype) -> bool,
            {
                itertools::multizip(($(<$param as QueryFetch>::fetch_columns(storage, test_archetype).into_iter(),)*)).collect_vec()
            }

            #[allow(non_snake_case, unused)]
            fn iter(columns: &Self::Columns, last_run: Tick, this_run: Tick) -> impl Iterator<Item = (Entity, Self::Fetch)> {
                let ($($param,)*) = columns;
                itertools::multizip(($(<$param as QueryFetch>::iter($param, last_run, this_run),)*))
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

            fn test_archetype(archetype: &Archetype) -> bool {
                $(
                    <$param as QueryFetch>::test_archetype(archetype) &&
                )*
                true
            }

            #[allow(non_snake_case)]
            fn any_changed(fetch: &Self::Fetch) -> bool {
                let ($($param,)*) = fetch;
                $(
                    <$param as QueryFetch>::any_changed($param) ||
                )*
                false
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

pub struct Query<Q, F = ()>
where
    Q: QueryFetch + ?Sized,
    F: QueryFilter + ?Sized,
{
    columns: Vec<Q::Columns>,
    last_run: Tick,
    this_run: Tick,
    _fetch: PhantomData<Q>,
    _filter: PhantomData<F>,
}

impl<Q, F> Query<Q, F>
where
    Q: QueryFetch + ?Sized,
    F: QueryFilter + ?Sized,
{
    pub fn new(world: &World) -> Self {
        let storage = world.storage();
        let columns = Q::fetch_columns(storage, &|archetype| {
            Q::test_archetype(archetype) && F::test_archetype(archetype)
        });

        Self {
            columns,
            last_run: world.last_change_tick(),
            this_run: world.read_change_tick(),
            _fetch: PhantomData,
            _filter: PhantomData,
        }
    }

    pub fn get(&self, entity: Entity) -> Option<Q::Fetch> {
        for columns in &self.columns {
            let mut iter = Q::iter(columns, self.last_run, self.this_run);
            if let Some((_, fetch)) = iter.find(|(e, _)| *e == entity) {
                return Some(fetch);
            }
        }

        None
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.iter().map(|(entity, item)| {
            // paranoid drop the item so that the column doesn't stay locked
            // todo: remove?
            drop(item);
            entity
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = (Entity, Q::Fetch)> + '_ {
        self.columns
            .iter()
            .flat_map(|col| Q::iter(col, self.last_run, self.this_run))
    }

    pub fn iter_changed(&self) -> impl Iterator<Item = (Entity, Q::Fetch)> + '_ {
        self.iter().filter(|(_, fetch)| Q::any_changed(fetch))
    }
}
