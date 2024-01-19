use std::fmt::Debug;

use atomic_refcell::{AtomicRef, AtomicRefMut};

use crate::storage::{Archetype, Components, EntitySet};

use super::{entity::EntityId, storage::ComponentSet, Bundle, Component};

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
        if !self.withouts.is_clear()
            && self
                .withouts
                .intersection(&archetype.component_ids())
                .count()
                > 0
        {
            return false;
        }

        if !self.withs.is_clear()
            && self.withs.intersection(&archetype.component_ids()).count()
                != self.withs.ones().count()
        {
            return false;
        }

        if !self.reads.is_clear()
            && self.reads.intersection(&archetype.component_ids()).count()
                != self.reads.ones().count()
        {
            return false;
        }

        if !self.writes.is_clear()
            && self.writes.intersection(&archetype.component_ids()).count()
                != self.writes.ones().count()
        {
            return false;
        }

        true
    }
}

impl Debug for QueryAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryAccess")
            .field("reads", &self.reads.ones().collect::<Vec<_>>())
            .field("writes", &self.writes.ones().collect::<Vec<_>>())
            .field("withs", &self.withs.ones().collect::<Vec<_>>())
            .field("withouts", &self.withouts.ones().collect::<Vec<_>>())
            .finish()
    }
}

pub trait Queryable<'a, F = ()>
where
    F: QueryFilter<'a>,
{
    type Item: Bundle;
    type ItemRef: 'a + Send;

    fn fetch(entity: EntityId, components: &'a Components) -> Option<Self::ItemRef>;

    fn access() -> QueryAccess;
}

impl<'a, F> Queryable<'a, F> for ()
where
    F: QueryFilter<'a>,
{
    type Item = ();
    type ItemRef = ();

    fn fetch(_entity: EntityId, _components: &'a Components) -> Option<Self::ItemRef> {
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

    fn fetch(entity: EntityId, components: &'a Components) -> Option<Self::ItemRef> {
        let component = components
            .component_iter(entity)
            .find(|c| c.info.id == crate::static_id::<Self::Item>())?;
        Some(Ref {
            // SAFETY: `component` is a valid pointer to a `T` because `crate::static_id::<T>()` is the same as `Self::Item::id()`.
            component: unsafe { component.borrow_as_ref_unchecked() },
            _marker: std::marker::PhantomData,
        })
    }

    fn access() -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::from_iter([crate::static_id::<T>() as usize]),
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

    fn fetch(entity: EntityId, components: &'a Components) -> Option<Self::ItemRef> {
        let component = components
            .component_iter(entity)
            .find(|c| c.info.id == crate::static_id::<Self::Item>())?;
        Some(Mut {
            // SAFETY: `component` is a valid pointer to a `T` because `crate::static_id::<T>()` is the same as `Self::Item::id()`.
            component: unsafe { component.borrow_as_mut_unchecked() },
            _marker: std::marker::PhantomData,
        })
    }

    fn access() -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::default(),
            writes: ComponentSet::from_iter([crate::static_id::<T>() as usize]),
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
        ComponentSet::from_iter([crate::static_id::<T>() as usize])
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
        ComponentSet::from_iter([crate::static_id::<T>() as usize])
    }
}

pub struct QueryState<'a, Q, F>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    entities: EntitySet,
    _filter: std::marker::PhantomData<&'a (Q, F)>,
}

impl<'a, Q, F> QueryState<'a, Q, F>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub fn new(components: &Components) -> Self {
        let entities = components.entities_matching_access(&Q::access());

        QueryState {
            entities,
            _filter: std::marker::PhantomData,
        }
    }

    pub fn get(&self, entity: EntityId, components: &'a Components) -> Option<Q::ItemRef> {
        if self.entities.contains(entity as usize) {
            Q::fetch(entity, components)
        } else {
            None
        }
    }

    pub fn iter(&'a self, components: &'a Components) -> QueryIter<'a, Q, F> {
        let iter = {
            self.entities
                .ones()
                .filter_map(move |entity| Q::fetch(entity as EntityId, components))
        };

        QueryIter {
            iter: Box::new(iter),
            _filter: std::marker::PhantomData,
        }
    }
}

pub struct Query<'a, Q, F = ()>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    components: &'a Components,
    state: QueryState<'a, Q, F>,
}

impl<'a, Q, F> Query<'a, Q, F>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub fn new(components: &'a Components) -> Self {
        Query {
            components,
            state: QueryState::new(components),
        }
    }

    pub fn get(&self, entity: EntityId) -> Option<Q::ItemRef> {
        self.state.get(entity, self.components)
    }

    pub fn iter(&'a self) -> QueryIter<'a, Q, F> {
        self.state.iter(self.components)
    }
}

pub struct QueryIter<'a, Q, F>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    iter: Box<dyn Iterator<Item = Q::ItemRef> + 'a>,
    _filter: std::marker::PhantomData<(Q, F)>,
}

impl<'a, Q, F> Iterator for QueryIter<'a, Q, F>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    type Item = Q::ItemRef;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
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

            fn fetch(entity: EntityId, components: &'a Components) -> Option<Self::ItemRef> {
                let ($($name,)*) = ($({
                    $name::fetch(entity, components)?
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
                    reads.union_with(&access.reads);
                    writes.union_with(&access.writes);
                    withs.union_with(&access.withs);
                    withouts.union_with(&access.withouts);
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
                    all.union_with(&$name::withs());
                )*
                all
            }

            fn withouts() -> ComponentSet {
                let mut all = ComponentSet::default();
                $(
                    all.union_with(&$name::withouts());
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
