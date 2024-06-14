use std::{any::TypeId, marker::PhantomData, sync::Arc};

use itertools::Itertools;
use weaver_util::lock::SharedLock;

use crate::prelude::{Archetype, ColumnRef, Data, SparseSet, Storage};

use super::{
    component::Component,
    entity::Entity,
    storage::{Mut, Ref},
    world::World,
};

pub enum QueryAccess {
    ReadOnly,
    ReadWrite,
}

pub struct ColumnIter<T: Component> {
    column: SharedLock<SparseSet<Data>>,
    index: usize,
    _marker: PhantomData<T>,
}

impl<T: Component> Iterator for ColumnIter<T> {
    type Item = (Entity, Ref<T>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.column.read().len() {
            return None;
        }

        let column = self.column.read_arc();
        let entity = column.sparse_index_of(self.index)?;
        let entity = Entity::from_usize(entity);
        let item = Ref::new(self.index, column);

        self.index += 1;

        Some((entity, item))
    }
}

pub struct ColumnMutIter<T: Component> {
    column: SharedLock<SparseSet<Data>>,
    index: usize,
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
        let item = Mut::new(self.index, column);
        self.index += 1;

        Some((entity, item))
    }
}

pub trait ColumnExt<T: Component> {
    fn dense_iter(&self) -> ColumnIter<T>;
    fn dense_iter_mut(&self) -> ColumnMutIter<T>;
}

impl<T: Component> ColumnExt<T> for ColumnRef {
    fn dense_iter(&self) -> ColumnIter<T> {
        ColumnIter {
            column: (*self).clone(),
            index: 0,
            _marker: PhantomData,
        }
    }

    fn dense_iter_mut(&self) -> ColumnMutIter<T> {
        ColumnMutIter {
            column: (*self).clone(),
            index: 0,
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
    fn iter(columns: &Self::Column) -> impl Iterator<Item = (Entity, Self::Fetch)>;
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

    fn iter(columns: &Self::Column) -> impl Iterator<Item = (Entity, Self::Fetch)> {
        columns.dense_iter()
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

    fn iter(columns: &Self::Column) -> impl Iterator<Item = (Entity, Self::Fetch)> {
        columns.dense_iter_mut()
    }
}

pub trait QueryFetch<'a> {
    type Columns;
    type Fetch;
    fn access() -> &'static [(TypeId, QueryAccess)];
    fn fetch_columns<F>(storage: &Storage, test_archetype: &F) -> Vec<Self::Columns>
    where
        F: Fn(&Archetype) -> bool;
    fn iter(columns: &Self::Columns) -> impl Iterator<Item = (Entity, Self::Fetch)>;
    fn test_archetype(archetype: &Archetype) -> bool;
}

impl<'a, T: Component> QueryFetch<'a> for &T {
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

    fn iter(columns: &Self::Columns) -> impl Iterator<Item = (Entity, Self::Fetch)> {
        columns.dense_iter()
    }

    fn test_archetype(archetype: &Archetype) -> bool {
        archetype.contains_component_by_type_id(TypeId::of::<T>())
    }
}

impl<'a, T: Component> QueryFetch<'a> for &mut T {
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

    fn iter(columns: &Self::Columns) -> impl Iterator<Item = (Entity, Self::Fetch)> {
        columns.dense_iter_mut()
    }

    fn test_archetype(archetype: &Archetype) -> bool {
        archetype.contains_component_by_type_id(TypeId::of::<T>())
    }
}

macro_rules! impl_query_fetch {
    ($($param:ident),*) => {
        impl<'a, $($param: QueryFetchParam<Column = ColumnRef> + QueryFetch<'a>),*> QueryFetch<'a> for ($($param,)*) {
            type Columns = ($($param::Column,)*);
            type Fetch = ($(
                <$param as QueryFetchParam>::Fetch,
                )*);

            fn access() -> &'static [(TypeId, QueryAccess)] {
                static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryAccess)>> = std::sync::OnceLock::new();
                ACCESS.get_or_init(|| vec![$((<$param as QueryFetchParam>::type_id(), <$param as QueryFetchParam>::access()),)*])
            }

            fn fetch_columns<Test>(storage: &Storage, test_archetype: &Test) -> Vec<Self::Columns>
            where
                Test: Fn(&Archetype) -> bool,
            {
                itertools::multizip(($(<$param as QueryFetchParam>::fetch_columns(storage, test_archetype).into_iter(),)*)).collect_vec()
            }

            #[allow(non_snake_case, unused)]
            fn iter(columns: &Self::Columns) -> impl Iterator<Item = (Entity, Self::Fetch)> {
                let ($($param,)*) = columns;
                itertools::multizip(($(<$param as QueryFetchParam>::iter($param),)*))
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
                    <$param as QueryFetch<'a>>::test_archetype(archetype) &&
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

pub trait QueryFilter {
    fn test_archetype(archetype: &Archetype) -> bool;
}

impl QueryFilter for () {
    fn test_archetype(_: &Archetype) -> bool {
        true
    }
}

macro_rules! impl_query_filter {
    ($($param:ident),*) => {
        impl<'a, $($param: QueryFetch<'a>),*> QueryFilter for ($($param,)*) {
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

pub struct Query<'a, Q, F = ()>
where
    Q: QueryFetch<'a> + ?Sized,
    F: QueryFilter + ?Sized,
{
    columns: Vec<Q::Columns>,
    _fetch: PhantomData<Q>,
    _filter: PhantomData<F>,
}

impl<'a, Q, F> Query<'a, Q, F>
where
    Q: QueryFetch<'a> + ?Sized,
    F: QueryFilter + ?Sized,
{
    pub fn new(world: &Arc<World>) -> Self {
        let storage = world.storage().read();
        let columns = Q::fetch_columns(&storage, &|archetype| {
            Q::test_archetype(archetype) && F::test_archetype(archetype)
        });

        Self {
            columns,
            _fetch: PhantomData,
            _filter: PhantomData,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (Entity, Q::Fetch)> + '_ {
        self.columns.iter().flat_map(Q::iter)
    }
}
