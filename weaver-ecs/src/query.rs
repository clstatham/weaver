use atomic_refcell::{AtomicRef, AtomicRefMut};
use std::fmt::Debug;
use weaver_proc_macro::all_tuples;

use crate::{
    component::LockedData,
    id::{DynamicId, IdRegistry},
    storage::{Archetype, ComponentMap, ComponentSet, Components, SparseSet},
};

use super::{bundle::Bundle, component::Component};

pub struct Ref<'a, T>
where
    T: Component,
{
    component: AtomicRef<'a, T>,
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

    fn fetch(
        data: &'a ComponentMap<&'a LockedData>,
        registry: &'a IdRegistry,
    ) -> Option<Self::ItemRef>;

    fn access(registry: &IdRegistry) -> QueryAccess;
}

impl<'a, F> Queryable<'a, F> for ()
where
    F: QueryFilter<'a>,
{
    type Item = ();
    type ItemRef = ();

    fn fetch(
        _data: &'a ComponentMap<&'a LockedData>,
        _registry: &'a IdRegistry,
    ) -> Option<Self::ItemRef> {
        Some(())
    }

    fn access(registry: &IdRegistry) -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::default(),
            writes: ComponentSet::default(),
            withs: F::withs(registry),
            withouts: F::withouts(registry),
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

    #[inline(never)]
    fn fetch(
        data: &'a ComponentMap<&'a LockedData>,
        registry: &IdRegistry,
    ) -> Option<Self::ItemRef> {
        let data = data.get(&registry.get_static::<T>())?;
        let data = data.borrow();

        Some(Ref {
            component: AtomicRef::map(data, |data| data.get_as::<T>().unwrap()),
        })
    }

    fn access(registry: &IdRegistry) -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::from_iter([registry.get_static::<T>()]),
            writes: ComponentSet::default(),
            withs: F::withs(registry),
            withouts: F::withouts(registry),
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

    #[inline(never)]
    fn fetch(
        data: &'a ComponentMap<&'a LockedData>,
        registry: &'a IdRegistry,
    ) -> Option<Self::ItemRef> {
        let data = data.get(&registry.get_static::<T>())?;
        let data = data.borrow_mut();

        Some(Mut {
            component: AtomicRefMut::map(data, |data| data.get_as_mut::<T>().unwrap()),
        })
    }

    fn access(registry: &IdRegistry) -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::default(),
            writes: ComponentSet::from_iter([registry.get_static::<T>()]),
            withs: F::withs(registry),
            withouts: F::withouts(registry),
        }
    }
}

/// Very similar to a Queryable, but instead of yielding a reference to the component, it is just used for filtering.
pub trait QueryFilter<'a> {
    fn withs(_registry: &IdRegistry) -> ComponentSet {
        ComponentSet::default()
    }
    fn withouts(_registry: &IdRegistry) -> ComponentSet {
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
    fn withs(registry: &IdRegistry) -> ComponentSet {
        ComponentSet::from_iter([registry.get_static::<T>()])
    }
}

pub struct Without<'a, T>(std::marker::PhantomData<&'a T>)
where
    T: Component;

impl<'a, T> QueryFilter<'a> for Without<'a, T>
where
    T: Component,
{
    fn withouts(registry: &IdRegistry) -> ComponentSet {
        ComponentSet::from_iter([registry.get_static::<T>()])
    }
}

pub struct Query<'a, Q, F = ()>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    registry: &'a IdRegistry,
    entries: SparseSet<DynamicId, ComponentMap<&'a LockedData>>,
    _marker: std::marker::PhantomData<(Q, F)>,
}

impl<'a, Q, F> Query<'a, Q, F>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub fn new(components: &'a Components) -> Self {
        let registry = components.registry();
        let entries = components
            .components_matching_access(Q::access(registry))
            .collect();
        Query {
            registry,
            entries,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn get(&'a self, entity: DynamicId) -> Option<Q::ItemRef> {
        let data = self.entries.get(&entity)?;
        Q::fetch(data, self.registry)
    }

    pub fn iter(&'a self) -> impl Iterator<Item = Q::ItemRef> + 'a {
        self.entries
            .iter()
            .filter_map(move |(_, data)| Q::fetch(data, self.registry))
    }
}

macro_rules! impl_queryable_for_tuple {
    ($($name:ident),*) => {
        #[allow(non_snake_case)]
        impl<'a, $($name),*, Filter> Queryable<'a, Filter> for ($($name,)*)
        where
            $($name: Queryable<'a, Filter>,)*
            Filter: QueryFilter<'a>,
            ($($name::Item,)*) : Bundle,
        {
            type Item = ($($name::Item,)*);
            type ItemRef = ($($name::ItemRef,)*);

            fn fetch(data: &'a ComponentMap<&'a LockedData>, registry: &'a IdRegistry) -> Option<Self::ItemRef> {
                Some(($($name::fetch(data, registry)?,)*))
            }

            fn access(registry: &IdRegistry) -> QueryAccess {
                let mut reads = ComponentSet::default();
                let mut writes = ComponentSet::default();
                let mut withs = ComponentSet::default();
                let mut withouts = ComponentSet::default();

                $({
                    let access = $name::access(registry);
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

all_tuples!(1..=16, impl_queryable_for_tuple);

macro_rules! impl_queryfilter_for_tuple {
    ($($name:ident),*) => {
        impl<'a, $($name),*> QueryFilter<'a> for ($($name,)*)
        where
            $($name: QueryFilter<'a>,)*
        {
            fn withs(registry: &IdRegistry) -> ComponentSet {
                let mut all = ComponentSet::default();
                $(
                    all.extend(&$name::withs(registry));
                )*
                all
            }

            fn withouts(registry: &IdRegistry) -> ComponentSet {
                let mut all = ComponentSet::default();
                $(
                    all.extend(&$name::withouts(registry));
                )*
                all
            }
        }
    };
}

all_tuples!(1..=16, impl_queryfilter_for_tuple);
