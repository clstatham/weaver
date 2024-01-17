use std::sync::Arc;

use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};
use rustc_hash::FxHashMap;

use crate::storage::{ComponentMap, EntityComponentsMap, EntitySet};

use super::{
    entity::EntityId,
    storage::{ComponentSet, Components, QueryMap},
    world::ComponentPtr,
    Bundle, Component, World,
};

struct QueryAccess {
    reads: ComponentSet,
    writes: ComponentSet,
    withs: ComponentSet,
    withouts: ComponentSet,
}

impl QueryAccess {
    fn matches_archetype(&self, archetype: &ComponentSet) -> bool {
        let mut includes = ComponentSet::default();

        includes.extend(&self.reads);
        includes.extend(&self.writes);
        includes.extend(&self.withs);

        let mut filtered = archetype.clone();

        filtered.retain(|component_id| !self.withouts.contains(component_id));
        filtered.retain(|component_id| includes.contains(component_id));

        filtered == includes
    }
}

pub trait Queryable<'a, F = ()>
where
    F: QueryFilter<'a>,
{
    type Item: Bundle;
    type ItemRef: 'a + Send;

    // Gets the item from the given components, if it exists.
    fn get(entries: &'a ComponentMap) -> Option<Self::ItemRef>;

    fn reads() -> Option<ComponentSet> {
        None
    }
    fn writes() -> Option<ComponentSet> {
        None
    }
    fn withs() -> Option<ComponentSet> {
        F::withs()
    }
    fn withouts() -> Option<ComponentSet> {
        F::withouts()
    }
    fn maybes() -> Option<ComponentSet> {
        None
    }
}

/// Collects the components that match the query, based on the given entities.
#[inline(never)]
fn collect_entities<'a, T: Queryable<'a, F>, F: QueryFilter<'a>>(
    components: &Components,
) -> EntitySet {
    let reads = T::reads().unwrap_or_default();
    let writes = T::writes().unwrap_or_default();
    let withs = T::withs().unwrap_or_default();
    let withouts = T::withouts().unwrap_or_default();

    let access = QueryAccess {
        reads,
        writes,
        withs,
        withouts,
    };

    let mut required = ComponentSet::default();
    required.extend(&access.reads);
    required.extend(&access.writes);

    let matching_archetypes = components
        .archetypes
        .iter()
        .filter_map(|(archetype, entities)| {
            if !entities.is_empty() && access.matches_archetype(archetype) {
                Some(entities)
            } else {
                None
            }
        });

    let mut entries = EntitySet::default();

    for entities in matching_archetypes {
        for &entity in entities.iter() {
            let components = components.entity_components.get(&entity).unwrap();

            if required.iter().all(|id| components.read().contains_key(id)) {
                entries.insert(entity);
            }
        }
    }

    entries
}

impl<'a, T, F> Queryable<'a, F> for &'a T
where
    T: Component,
    F: QueryFilter<'a>,
{
    type Item = T;
    type ItemRef = MappedRwLockReadGuard<'a, T>;

    fn get(entries: &'a ComponentMap) -> Option<Self::ItemRef> {
        entries.get(&T::static_id()).map(|component| {
            RwLockReadGuard::map(component.component.read(), |component| {
                (*component).as_any().downcast_ref::<T>().unwrap()
            })
        })
    }

    fn reads() -> Option<ComponentSet> {
        Some(ComponentSet::from_iter(vec![T::static_id()]))
    }
}

impl<'a, T, F> Queryable<'a, F> for &'a mut T
where
    T: Component,
    F: QueryFilter<'a>,
{
    type Item = T;
    type ItemRef = MappedRwLockWriteGuard<'a, T>;
    fn get(entries: &'a ComponentMap) -> Option<Self::ItemRef> {
        entries.get(&T::static_id()).map(|component| {
            RwLockWriteGuard::map(component.component.write(), |component| {
                (*component).as_any_mut().downcast_mut::<T>().unwrap()
            })
        })
    }

    fn writes() -> Option<ComponentSet> {
        Some(ComponentSet::from_iter(vec![T::static_id()]))
    }
}

/// Very similar to a Queryable, but instead of yielding a reference to the component, it is just used for filtering.
pub trait QueryFilter<'a> {
    fn withs() -> Option<ComponentSet> {
        None
    }
    fn withouts() -> Option<ComponentSet> {
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
    fn withs() -> Option<ComponentSet> {
        Some(ComponentSet::from_iter(vec![T::static_id()]))
    }
}

pub struct Without<'a, T>(std::marker::PhantomData<&'a T>)
where
    T: Component;

impl<'a, T> QueryFilter<'a> for Without<'a, T>
where
    T: Component,
{
    fn withouts() -> Option<ComponentSet> {
        Some(ComponentSet::from_iter(vec![T::static_id()]))
    }
}

pub struct Query<'a, T, F = ()>
where
    T: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub(crate) entries: FxHashMap<EntityId, ComponentMap>,

    _phantom: std::marker::PhantomData<&'a (T, F)>,
}

impl<'a, T, F> Query<'a, T, F>
where
    T: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub fn new(components: RwLockReadGuard<'a, Components>) -> Self {
        let entities = collect_entities::<T, F>(&components);

        let mut required = T::reads().unwrap_or_default();
        required.extend(&T::writes().unwrap_or_default());

        let mut entries = FxHashMap::default();

        for entity in entities {
            let components = components.entity_components.get(&entity).unwrap();

            for &id in required.iter() {
                if let Some(component) = components.read().get(&id) {
                    entries
                        .entry(entity)
                        .or_insert_with(ComponentMap::default)
                        .insert(id, component.clone());
                }
            }
        }

        Self {
            entries,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn get(&'a self, entity: EntityId) -> Option<T::ItemRef> {
        self.entries
            .get(&entity)
            .and_then(|entries| T::get(entries))
    }

    pub fn entities(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.entries.keys().copied()
    }

    pub fn iter(&'a self) -> impl Iterator<Item = T::ItemRef> + '_ {
        self.entities().filter_map(move |entity| self.get(entity))
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
            fn withs() -> Option<ComponentSet> {
                let mut all = ComponentSet::default();
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

            fn withouts() -> Option<ComponentSet> {
                let mut all = ComponentSet::default();
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
