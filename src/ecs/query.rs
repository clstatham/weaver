use std::{
    cell::{Ref, RefCell, RefMut},
    fmt::Debug,
    sync::Arc,
};

use rustc_hash::{FxHashMap, FxHashSet};

use super::{entity::Entity, world::Components, Bundle, Component};

#[derive(Clone)]
pub struct QueryEntry {
    entity: Entity,
    component: Arc<RefCell<dyn Component>>,
}

impl Debug for QueryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryEntry")
            .field("entity", &self.entity)
            .finish()
    }
}

pub trait QueryFilter<'a> {
    type Item: Bundle;
    type ItemRef: 'a;

    /// Returns a set of entities that match the filter, based on their components.
    fn matching_entities(components: &Components) -> FxHashSet<Entity> {
        let mut matches = FxHashSet::default();

        let reads = Self::reads().unwrap_or_default();
        let writes = Self::writes().unwrap_or_default();

        for (&entity, components) in components.iter() {
            let mut has_all_reads = true;
            for read in &reads {
                if !components.contains_key(read) {
                    has_all_reads = false;
                    break;
                }
            }
            if !has_all_reads {
                continue;
            }

            let mut has_all_writes = true;
            for write in &writes {
                if !components.contains_key(write) {
                    has_all_writes = false;
                    break;
                }
            }
            if !has_all_writes {
                continue;
            }

            matches.insert(entity);
        }

        matches
    }

    /// Filters the components based on the filter, returning a list of entries.
    fn filter(components: &Components) -> Vec<QueryEntry> {
        let entities = Self::matching_entities(components);
        entities
            .into_iter()
            .flat_map(|entity| {
                components
                    .get(&entity)
                    .unwrap()
                    .iter()
                    .map(move |(component_id, component)| QueryEntry {
                        entity,
                        component: component.clone(),
                    })
            })
            .collect()
    }

    // Gets the item from the given entity, if it exists.
    fn get(entity: Entity, entries: &'a [QueryEntry]) -> Option<Self::ItemRef>;

    fn reads() -> Option<FxHashSet<u64>> {
        None
    }
    fn writes() -> Option<FxHashSet<u64>> {
        None
    }
    fn withs() -> Option<FxHashSet<u64>> {
        None
    }
    fn withouts() -> Option<FxHashSet<u64>> {
        None
    }
    fn ors() -> Option<FxHashSet<(u64, u64)>> {
        None
    }
    fn maybes() -> Option<FxHashSet<u64>> {
        None
    }
}

impl<'a, T> QueryFilter<'a> for &'a T
where
    T: Component,
{
    type Item = T;
    type ItemRef = Ref<'a, T>;

    fn get(entity: Entity, entries: &'a [QueryEntry]) -> Option<Self::ItemRef> {
        entries.iter().find_map(|entry| {
            if entry.entity == entity && entry.component.borrow().as_any().is::<T>() {
                Some(Ref::map(entry.component.borrow(), |component| {
                    component.as_any().downcast_ref::<T>().unwrap()
                }))
            } else {
                None
            }
        })
    }

    fn reads() -> Option<FxHashSet<u64>> {
        Some(FxHashSet::from_iter(vec![T::component_id()]))
    }
}

impl<'a, T> QueryFilter<'a> for &'a mut T
where
    T: Component,
{
    type Item = T;
    type ItemRef = RefMut<'a, T>;

    fn get(entity: Entity, entries: &'a [QueryEntry]) -> Option<Self::ItemRef> {
        entries.iter().find_map(|entry| {
            if entry.entity == entity && entry.component.borrow().as_any().is::<T>() {
                Some(RefMut::map(entry.component.borrow_mut(), |component| {
                    component.as_any_mut().downcast_mut::<T>().unwrap()
                }))
            } else {
                None
            }
        })
    }

    fn writes() -> Option<FxHashSet<u64>> {
        Some(FxHashSet::from_iter(vec![T::component_id()]))
    }
}

pub struct Query<'a, T>
where
    T: QueryFilter<'a>,
{
    // component id, entities/components
    pub(crate) entries: Vec<QueryEntry>,

    reads: FxHashSet<u64>,
    writes: FxHashSet<u64>,
    // withs: FxHashSet<u64>,
    // withouts: FxHashSet<u64>,
    // ors: FxHashSet<(u64, u64)>,
    // maybes: FxHashSet<u64>,
    _phantom: std::marker::PhantomData<&'a T>,
}

impl<'a, T> Query<'a, T>
where
    T: QueryFilter<'a>,
{
    pub(crate) fn new(components: &Components) -> Self {
        let entries = T::filter(components);

        let reads = T::reads().unwrap_or_default();
        let writes = T::writes().unwrap_or_default();
        // let withs = T::withs().unwrap_or_default();
        // let withouts = T::withouts().unwrap_or_default();
        // let ors = T::ors().unwrap_or_default();
        // let maybes = T::maybes().unwrap_or_default();

        Self {
            entries,

            reads,
            writes,
            // withs,
            // withouts,
            // ors,
            // maybes,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn get(&'a self, entity: Entity) -> Option<T::ItemRef> {
        T::get(entity, &self.entries)
    }

    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        let entities = self.entries.iter().map(|entry| entry.entity);
        FxHashSet::from_iter(entities).into_iter()
    }

    pub fn iter(&'a self) -> Box<dyn Iterator<Item = T::ItemRef> + '_> {
        Box::new(self.entities().filter_map(move |entity| self.get(entity)))
    }
}

weaver_proc_macro::impl_query_for_n_tuple!(2);
weaver_proc_macro::impl_query_for_n_tuple!(3);
weaver_proc_macro::impl_query_for_n_tuple!(4);
weaver_proc_macro::impl_query_for_n_tuple!(5);
weaver_proc_macro::impl_query_for_n_tuple!(6);
weaver_proc_macro::impl_query_for_n_tuple!(7);
weaver_proc_macro::impl_query_for_n_tuple!(8);
