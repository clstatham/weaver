use std::fmt::Debug;

use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLockReadGuard, RwLockWriteGuard,
};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{entity::EntityId, storage::Components, world::ComponentPtr, Bundle, Component, World};

struct QueryAccess {
    reads: FxHashSet<usize>,
    writes: FxHashSet<usize>,
    withs: FxHashSet<usize>,
    withouts: FxHashSet<usize>,
}

impl QueryAccess {
    fn matches_archetype(&self, archetype: &FxHashSet<usize>) -> bool {
        let mut includes = FxHashSet::<usize>::default();

        includes.extend(&self.reads);
        includes.extend(&self.writes);
        includes.extend(&self.withs);

        let mut filtered = archetype.clone();

        filtered.retain(|component_id| includes.contains(component_id));
        filtered.retain(|component_id| !self.withouts.contains(component_id));

        filtered == includes
    }
}

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
    fn collect(components: &Components) -> FxHashMap<EntityId, Vec<QueryEntry>> {
        // let mut entries: FxHashMap<Entity, Vec<QueryEntry>> = FxHashMap::default();

        let reads = Self::reads().unwrap_or_default();
        let writes = Self::writes().unwrap_or_default();
        let withs = Self::withs().unwrap_or_default();
        let withouts = Self::withouts().unwrap_or_default();

        let access = QueryAccess {
            reads,
            writes,
            withs,
            withouts,
        };

        let required: FxHashSet<usize> = access.reads.union(&access.writes).copied().collect();

        let mut entries = FxHashMap::default();

        let matching_archetypes =
            components
                .archetypes
                .iter()
                .filter_map(|(archetype, entities)| {
                    if !entities.is_empty() {
                        let matches = access.matches_archetype(archetype);
                        if matches {
                            Some(entities)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });

        for entities in matching_archetypes {
            let these_entries = entities
                .par_iter()
                .filter_map(|entity| {
                    let mut entry = Vec::with_capacity(required.len());
                    for component_id in required.iter() {
                        let component = components
                            .entity_components
                            .get(entity)
                            .and_then(|components| components.get(component_id));
                        if let Some(component) = component {
                            entry.push(QueryEntry {
                                entity: *entity,
                                component: component.clone(),
                            });
                        } else {
                            break;
                        }
                    }
                    if entry.len() == required.len() {
                        Some((*entity, entry))
                    } else {
                        None
                    }
                })
                .collect::<FxHashMap<_, _>>();

            entries.extend(these_entries);
        }

        entries
    }

    // Gets the item from the given entity, if it exists.
    fn get(entity: EntityId, entries: &'a [QueryEntry]) -> Option<Self::ItemRef>;

    fn reads() -> Option<FxHashSet<usize>> {
        None
    }
    fn writes() -> Option<FxHashSet<usize>> {
        None
    }
    fn withs() -> Option<FxHashSet<usize>> {
        F::withs()
    }
    fn withouts() -> Option<FxHashSet<usize>> {
        F::withouts()
    }
    fn maybes() -> Option<FxHashSet<usize>> {
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

    fn reads() -> Option<FxHashSet<usize>> {
        Some(FxHashSet::from_iter(vec![T::static_id()]))
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

    fn writes() -> Option<FxHashSet<usize>> {
        Some(FxHashSet::from_iter(vec![T::static_id()]))
    }
}

/// Very similar to a Queryable, but instead of yielding a reference to the component, it is just used for filtering.
pub trait QueryFilter<'a> {
    fn withs() -> Option<FxHashSet<usize>> {
        None
    }
    fn withouts() -> Option<FxHashSet<usize>> {
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
    fn withs() -> Option<FxHashSet<usize>> {
        Some(FxHashSet::from_iter(vec![T::static_id()]))
    }
}

pub struct Without<'a, T>(std::marker::PhantomData<&'a T>)
where
    T: Component;

impl<'a, T> QueryFilter<'a> for Without<'a, T>
where
    T: Component,
{
    fn withouts() -> Option<FxHashSet<usize>> {
        Some(FxHashSet::from_iter(vec![T::static_id()]))
    }
}

pub struct Query<'a, T, F = ()>
where
    T: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub(crate) entries: FxHashMap<EntityId, Vec<QueryEntry>>,

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
            .get(&entity)
            .and_then(|entries| T::get(entity, entries))
    }

    pub fn entities(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.entries.keys().copied()
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
            fn withs() -> Option<FxHashSet<usize>> {
                let mut all = FxHashSet::default();
                $(
                    if let Some(withs) = $name::withs() {
                        all.extend(&withs);
                    }
                )*
                if all.is_empty() {
                    return None;
                }
                Some(all)
            }

            fn withouts() -> Option<FxHashSet<usize>> {
                let mut all = FxHashSet::default();
                $(
                    if let Some(withouts) = $name::withouts() {
                        all.extend(&withouts);
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
