use std::fmt::Debug;

use bit_set::BitSet;
use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLockReadGuard, RwLockWriteGuard,
};
use rayon::prelude::*;

use super::{
    entity::EntityId,
    storage::{Components, SparseSet},
    world::ComponentPtr,
    Bundle, Component, World,
};

#[derive(Clone)]
pub struct QueryEntry {
    entity: EntityId,
    component: ComponentPtr,
}

impl Debug for QueryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryEntry")
            .field("entity", &self.entity)
            .field("component_name", &self.component.component_name)
            .field("component_id", &self.component.component_id)
            .finish()
    }
}

pub trait Queryable<'a, F = ()>
where
    F: QueryFilter<'a>,
{
    type Item: Bundle;
    type ItemRef: 'a + Send;

    /// Collects the components that match the query, based on the given entities.
    fn collect(components: &Components) -> SparseSet<Vec<QueryEntry>, EntityId> {
        // let mut entries: FxHashMap<Entity, Vec<QueryEntry>> = FxHashMap::default();

        let reads = Self::reads().unwrap_or_default();
        let writes = Self::writes().unwrap_or_default();
        let withs = Self::withs().unwrap_or_default();
        let withouts = Self::withouts().unwrap_or_default();
        let maybes = Self::maybes().unwrap_or_default();

        let entries = components
            .par_iter()
            .filter_map(|(&entity, components)| {
                // check if the entity has the right combination of components
                // it needs to have ALL of the components in `reads`, `writes`, and `withs`
                // it needs to have NONE of the components in `withouts`
                // we don't care about the maybes, they're optional anyway

                let component_ids = components.indices().collect::<BitSet<_>>();

                // check if the entity has all of the required components
                if !reads.is_subset(&component_ids)
                    || !writes.is_subset(&component_ids)
                    || !withs.is_subset(&component_ids)
                {
                    return None;
                }

                // check if the entity has any of the excluded components
                if !withouts.is_disjoint(&component_ids) {
                    return None;
                }

                // gather the matching components
                let matching_components = components
                    .iter()
                    .filter_map(|(&component_id, component)| {
                        // we care about the maybes here, since they're always included in the query (just wrapped in an Option)
                        if reads.contains(component_id)
                            || writes.contains(component_id)
                            || maybes.contains(component_id)
                        {
                            Some(QueryEntry {
                                entity,
                                component: component.clone(),
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                Some((entity, matching_components))
            })
            .collect();

        entries
    }

    // Gets the item from the given entity, if it exists.
    fn get(entity: EntityId, entries: &'a [QueryEntry]) -> Option<Self::ItemRef>;

    fn reads() -> Option<BitSet> {
        None
    }
    fn writes() -> Option<BitSet> {
        None
    }
    fn withs() -> Option<BitSet> {
        F::withs()
    }
    fn withouts() -> Option<BitSet> {
        F::withouts()
    }
    fn ors() -> Option<BitSet<(u64, u64)>> {
        None
    }
    fn maybes() -> Option<BitSet> {
        None
    }
}

impl<'a, T, F> Queryable<'a, F> for &'a T
where
    T: Component,
    F: QueryFilter<'a>,
{
    type Item = T;
    type ItemRef = MappedRwLockReadGuard<'a, T>;

    fn get(entity: EntityId, entries: &'a [QueryEntry]) -> Option<Self::ItemRef> {
        entries.iter().find_map(|entry| {
            if entry.entity == entity && entry.component.component_id == T::static_id() {
                Some(RwLockReadGuard::map(
                    entry.component.component.read(),
                    |component| component.as_any().downcast_ref::<T>().unwrap(),
                ))
            } else {
                None
            }
        })
    }

    fn reads() -> Option<BitSet> {
        Some(BitSet::from_iter(vec![T::static_id()]))
    }
}

impl<'a, T, F> Queryable<'a, F> for &'a mut T
where
    T: Component,
    F: QueryFilter<'a>,
{
    type Item = T;
    type ItemRef = MappedRwLockWriteGuard<'a, T>;

    fn get(entity: EntityId, entries: &'a [QueryEntry]) -> Option<Self::ItemRef> {
        entries.iter().find_map(|entry| {
            if entry.entity == entity && entry.component.component_id == T::static_id() {
                Some(RwLockWriteGuard::map(
                    entry.component.component.write(),
                    |component| component.as_any_mut().downcast_mut::<T>().unwrap(),
                ))
            } else {
                None
            }
        })
    }

    fn writes() -> Option<BitSet> {
        Some(BitSet::from_iter(vec![T::static_id()]))
    }
}

/// Very similar to a Queryable, but instead of yielding a reference to the component, it is just used for filtering.
pub trait QueryFilter<'a> {
    fn withs() -> Option<BitSet> {
        None
    }
    fn withouts() -> Option<BitSet> {
        None
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
    fn withs() -> Option<BitSet> {
        Some(BitSet::from_iter(vec![T::static_id()]))
    }
}

pub struct Without<'a, T>(std::marker::PhantomData<&'a T>)
where
    T: Component;

impl<'a, T> QueryFilter<'a> for Without<'a, T>
where
    T: Component,
{
    fn withouts() -> Option<BitSet> {
        Some(BitSet::from_iter(vec![T::static_id()]))
    }
}

pub struct Query<'a, T, F = ()>
where
    T: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub(crate) entries: SparseSet<Vec<QueryEntry>, EntityId>,

    _phantom: std::marker::PhantomData<&'a (T, F)>,
}

impl<'a, T, F> Query<'a, T, F>
where
    T: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub(crate) fn new(world: &World) -> Self {
        let entries = T::collect(&world.components.read());

        Self {
            entries,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn get(&'a self, entity: EntityId) -> Option<T::ItemRef> {
        self.entries
            .get(entity)
            .and_then(|entries| T::get(entity, entries))
    }

    pub fn entities(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.entries.indices()
    }

    pub fn iter(&'a self) -> impl Iterator<Item = T::ItemRef> + '_ {
        self.entries
            .iter()
            .filter_map(move |(&entity, entries)| T::get(entity, entries))
    }

    pub fn par_iter(&'a self) -> impl ParallelIterator<Item = T::ItemRef> + '_ {
        self.entries
            .par_iter()
            .filter_map(move |(&entity, entries)| T::get(entity, entries))
    }
}

weaver_proc_macro::impl_queryable_for_n_tuple!(2);
weaver_proc_macro::impl_queryable_for_n_tuple!(3);
weaver_proc_macro::impl_queryable_for_n_tuple!(4);

macro_rules! impl_queryfilter_for_tuple {
    ($($name:ident),*) => {
        impl<'a, $($name),*> QueryFilter<'a> for ($($name,)*)
        where
            $($name: QueryFilter<'a>,)*
        {
            fn withs() -> Option<BitSet> {
                let mut all = BitSet::default();
                $(
                    if let Some(withs) = $name::withs() {
                        all.union_with(&withs);
                    }
                )*
                if all.is_empty() {
                    return None;
                }
                Some(all)
            }

            fn withouts() -> Option<BitSet> {
                let mut all = BitSet::default();
                $(
                    if let Some(withouts) = $name::withouts() {
                        all.union_with(&withouts);
                    }
                )*
                if all.is_empty() {
                    return None;
                }
                Some(all)
            }
        }
    };
}

impl_queryfilter_for_tuple!(A);
impl_queryfilter_for_tuple!(A, B);
impl_queryfilter_for_tuple!(A, B, C);
impl_queryfilter_for_tuple!(A, B, C, D);
