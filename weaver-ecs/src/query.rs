use atomic_refcell::{AtomicRef, AtomicRefMut};
use rayon::prelude::*;
use std::fmt::Debug;

use crate::storage::{Archetype, ComponentSet, Components};

use super::{entity::EntityId, Bundle, Component};

pub struct Ref<'a, T>
where
    T: Component,
{
    component: AtomicRef<'a, T>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T> std::ops::Deref for Ref<'a, T>
where
    T: Component,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.component
    }
}

pub struct Mut<'a, T>
where
    T: Component,
{
    component: AtomicRefMut<'a, T>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T> std::ops::Deref for Mut<'a, T>
where
    T: Component,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.component
    }
}

impl<'a, T> std::ops::DerefMut for Mut<'a, T>
where
    T: Component,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.component
    }
}

#[derive(Default)]
pub struct QueryAccess {
    pub reads: ComponentSet,
    pub writes: ComponentSet,
    pub withs: ComponentSet,
    pub withouts: ComponentSet,
}

impl QueryAccess {
    pub fn matches_archetype(&self, archetype: &Archetype) -> bool {
        if !self.withouts.is_empty()
            && self
                .withouts
                .intersection(&archetype.component_ids())
                .count()
                > 0
        {
            return false;
        }

        if !self.withs.is_empty()
            && self.withs.intersection(&archetype.component_ids()).count() != self.withs.len()
        {
            return false;
        }

        if !self.reads.is_empty()
            && self.reads.intersection(&archetype.component_ids()).count() != self.reads.len()
        {
            return false;
        }

        if !self.writes.is_empty()
            && self.writes.intersection(&archetype.component_ids()).count() != self.writes.len()
        {
            return false;
        }

        true
    }
}

impl Debug for QueryAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryAccess")
            .field("reads", &self.reads.iter().collect::<Vec<_>>())
            .field("writes", &self.writes.iter().collect::<Vec<_>>())
            .field("withs", &self.withs.iter().collect::<Vec<_>>())
            .field("withouts", &self.withouts.iter().collect::<Vec<_>>())
            .finish()
    }
}

pub trait Queryable<'a, F = ()>
where
    F: QueryFilter<'a>,
{
    type Item: Bundle;
    type ItemRef: 'a + Send;

    fn fetch(entity: EntityId, archetype: &'a Archetype) -> Option<Self::ItemRef>;

    fn access() -> QueryAccess;
}

impl<'a, F> Queryable<'a, F> for ()
where
    F: QueryFilter<'a>,
{
    type Item = ();
    type ItemRef = ();

    fn fetch(_entity: EntityId, _archetype: &'a Archetype) -> Option<Self::ItemRef> {
        Some(())
    }

    fn access() -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::default(),
            writes: ComponentSet::default(),
            withs: F::withs(),
            withouts: F::withouts(),
        }
    }
}

impl<'a, T, F> Queryable<'a, F> for &'a T
where
    T: Component,
    F: QueryFilter<'a>,
{
    type Item = T;
    type ItemRef = Ref<'a, T>;

    fn fetch(entity: EntityId, archetype: &'a Archetype) -> Option<Self::ItemRef> {
        let id = crate::static_id::<T>();
        let column = archetype.get_column(id)?;
        let component = AtomicRef::map(column, |column| unsafe {
            column.get(entity).unwrap().as_ref_unchecked()
        });
        Some(Ref {
            // SAFETY: `component` is a valid pointer to a `T` because `crate::static_id::<T>()` is the same as `Self::Item::id()`.
            component,
            _marker: std::marker::PhantomData,
        })
    }

    fn access() -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::from_iter([crate::static_id::<T>()]),
            writes: ComponentSet::default(),
            withs: F::withs(),
            withouts: F::withouts(),
        }
    }
}

impl<'a, T, F> Queryable<'a, F> for &'a mut T
where
    T: Component,
    F: QueryFilter<'a>,
{
    type Item = T;
    type ItemRef = Mut<'a, T>;

    fn fetch(entity: EntityId, archetype: &'a Archetype) -> Option<Self::ItemRef> {
        let id = crate::static_id::<T>();
        let column = archetype.get_column_mut(id)?;
        let component = AtomicRefMut::map(column, |column| unsafe {
            column.get_mut(entity).unwrap().as_mut_unchecked()
        });
        Some(Mut {
            // SAFETY: `component` is a valid pointer to a `T` because `crate::static_id::<T>()` is the same as `Self::Item::id()`.
            component,
            _marker: std::marker::PhantomData,
        })
    }

    fn access() -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::default(),
            writes: ComponentSet::from_iter([crate::static_id::<T>()]),
            withs: F::withs(),
            withouts: F::withouts(),
        }
    }
}

/// Very similar to a Queryable, but instead of yielding a reference to the component, it is just used for filtering.
pub trait QueryFilter<'a> {
    fn withs() -> ComponentSet {
        ComponentSet::default()
    }
    fn withouts() -> ComponentSet {
        ComponentSet::default()
    }
}

/// Default pass-through filter that yields all entries.
impl<'a> QueryFilter<'a> for () {}

pub struct With<'a, T>(std::marker::PhantomData<&'a T>)
where
    T: Component;

impl<'a, T> QueryFilter<'a> for With<'a, T>
where
    T: Component,
{
    fn withs() -> ComponentSet {
        ComponentSet::from_iter([crate::static_id::<T>()])
    }
}

pub struct Without<'a, T>(std::marker::PhantomData<&'a T>)
where
    T: Component;

impl<'a, T> QueryFilter<'a> for Without<'a, T>
where
    T: Component,
{
    fn withouts() -> ComponentSet {
        ComponentSet::from_iter([crate::static_id::<T>()])
    }
}

pub struct Query<'a, Q, F = ()>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    archetypes: Vec<&'a Archetype>,
    _marker: std::marker::PhantomData<(Q, F)>,
}

impl<'a, Q, F> Query<'a, Q, F>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub fn new(components: &'a Components) -> Self {
        let archetypes = components
            .archetypes
            .iter()
            .filter(|archetype| Q::access().matches_archetype(archetype))
            .collect();
        Query {
            archetypes,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn get(&self, entity: EntityId) -> Option<Q::ItemRef> {
        self.archetypes
            .iter()
            .filter_map(|archetype| Q::fetch(entity, archetype))
            .next()
    }

    pub fn iter(&'a self) -> impl Iterator<Item = Q::ItemRef> + 'a {
        self.archetypes.iter().flat_map(|archetype| {
            archetype
                .entities
                .ones()
                .filter_map(move |entity| Q::fetch(entity as EntityId, archetype))
        })
    }

    pub fn par_iter(&'a self) -> impl ParallelIterator<Item = Q::ItemRef> + Sync + 'a {
        self.archetypes.par_iter().flat_map(|archetype| {
            archetype
                .entities_hashset
                .par_iter()
                .filter_map(move |entity| Q::fetch(*entity, archetype))
        })
    }
}

macro_rules! impl_queryable_for_tuple {
    ($($name:ident),*) => {
        #[allow(non_snake_case)]
        impl<'a, $($name),*, F> Queryable<'a, F> for ($($name,)*)
        where
            $($name: Queryable<'a, F>,)*
            F: QueryFilter<'a>,
            ($($name::Item,)*) : Bundle,
        {
            type Item = ($($name::Item,)*);
            type ItemRef = ($($name::ItemRef,)*);

            fn fetch(entity: EntityId, archetype: &'a Archetype) -> Option<Self::ItemRef> {
                let ($($name,)*) = ($({
                    $name::fetch(entity, archetype)?
                },
                )*);
                Some(($($name,)*))
            }

            fn access() -> QueryAccess {
                let mut reads = ComponentSet::default();
                let mut writes = ComponentSet::default();
                let mut withs = ComponentSet::default();
                let mut withouts = ComponentSet::default();

                $({
                    let access = $name::access();
                    reads.extend(&access.reads);
                    writes.extend(&access.writes);
                    withs.extend(&access.withs);
                    withouts.extend(&access.withouts);
                })*

                QueryAccess {
                    reads,
                    writes,
                    withs,
                    withouts,
                }
            }
        }
    };
}

impl_queryable_for_tuple!(A);
impl_queryable_for_tuple!(A, B);
impl_queryable_for_tuple!(A, B, C);
impl_queryable_for_tuple!(A, B, C, D);

macro_rules! impl_queryfilter_for_tuple {
    ($($name:ident),*) => {
        impl<'a, $($name),*> QueryFilter<'a> for ($($name,)*)
        where
            $($name: QueryFilter<'a>,)*
        {
            fn withs() -> ComponentSet {
                let mut all = ComponentSet::default();
                $(
                    all.extend(&$name::withs());
                )*
                all
            }

            fn withouts() -> ComponentSet {
                let mut all = ComponentSet::default();
                $(
                    all.extend(&$name::withouts());
                )*
                all
            }
        }
    };
}

impl_queryfilter_for_tuple!(A);
impl_queryfilter_for_tuple!(A, B);
impl_queryfilter_for_tuple!(A, B, C);
impl_queryfilter_for_tuple!(A, B, C, D);
