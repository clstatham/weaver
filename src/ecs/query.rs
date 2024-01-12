use std::fmt::Debug;

use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLockReadGuard, RwLockWriteGuard,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{
    entity::Entity,
    world::{ComponentPtr, Components},
    Bundle, Component, World,
};

#[derive(Clone)]
pub struct QueryEntry {
    entity: Entity,
    component: ComponentPtr,
}

impl Debug for QueryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryEntry")
            .field("entity", &self.entity)
            .field("component_id", &self.component.component_id)
            .finish()
    }
}

pub trait Queryable<'a, F = ()>
where
    F: QueryFilter<'a>,
{
    type Item: Bundle;
    type ItemRef: 'a;

    /// Collects the components that match the query, based on the given entities.
    fn collect(components: &Components) -> FxHashMap<Entity, Vec<QueryEntry>> {
        let mut entries: FxHashMap<Entity, Vec<QueryEntry>> = FxHashMap::default();

        let reads = Self::reads().unwrap_or_default();
        let writes = Self::writes().unwrap_or_default();
        let withs = Self::withs().unwrap_or_default();
        let withouts = Self::withouts().unwrap_or_default();
        let maybes = Self::maybes().unwrap_or_default();

        for (&entity, components) in components.iter() {
            // check if the entity has the right combination of components
            // it needs to have ALL of the components in `reads`, `writes`, and `withs`
            // it needs to have NONE of the components in `withouts`
            // we don't care about the maybes, they're optional anyway

            let component_ids = components.keys().copied().collect::<FxHashSet<_>>();

            // check if the entity has all of the required components
            if !reads.is_subset(&component_ids)
                || !writes.is_subset(&component_ids)
                || !withs.is_subset(&component_ids)
            {
                continue;
            }

            // check if the entity has any of the excluded components
            if !withouts.is_disjoint(&component_ids) {
                continue;
            }

            // gather the matching components
            let mut matching_components = Vec::new();

            for (&component_id, component) in components {
                // we care about the maybes here, since they're always included in the query (just wrapped in an Option)
                if reads.contains(&component_id)
                    || writes.contains(&component_id)
                    || maybes.contains(&component_id)
                {
                    matching_components.push(QueryEntry {
                        entity,
                        component: component.clone(),
                    });
                }
            }

            entries.insert(entity, matching_components);
        }

        entries
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
        F::withs()
    }
    fn withouts() -> Option<FxHashSet<u64>> {
        F::withouts()
    }
    fn ors() -> Option<FxHashSet<(u64, u64)>> {
        None
    }
    fn maybes() -> Option<FxHashSet<u64>> {
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

    fn get(entity: Entity, entries: &'a [QueryEntry]) -> Option<Self::ItemRef> {
        entries.iter().find_map(|entry| {
            if entry.entity == entity && entry.component.component_id == T::component_id() {
                Some(RwLockReadGuard::map(
                    entry.component.component.read(),
                    |component| component.as_any().downcast_ref::<T>().unwrap(),
                ))
            } else {
                None
            }
        })
    }

    fn reads() -> Option<FxHashSet<u64>> {
        Some(FxHashSet::from_iter(vec![T::component_id()]))
    }
}

impl<'a, T, F> Queryable<'a, F> for &'a mut T
where
    T: Component,
    F: QueryFilter<'a>,
{
    type Item = T;
    type ItemRef = MappedRwLockWriteGuard<'a, T>;

    fn get(entity: Entity, entries: &'a [QueryEntry]) -> Option<Self::ItemRef> {
        entries.iter().find_map(|entry| {
            if entry.entity == entity && entry.component.component_id == T::component_id() {
                Some(RwLockWriteGuard::map(
                    entry.component.component.write(),
                    |component| component.as_any_mut().downcast_mut::<T>().unwrap(),
                ))
            } else {
                None
            }
        })
    }

    fn writes() -> Option<FxHashSet<u64>> {
        Some(FxHashSet::from_iter(vec![T::component_id()]))
    }
}

/// A query that always matches, returning an Option<T> where T is the queried component.
/// If the component exists, it will be Some(T), otherwise it will be None.
impl<'a, T, F> Queryable<'a, F> for Option<T>
where
    T: Queryable<'a, F>,
    <T as Queryable<'a, F>>::Item: Component,
    F: QueryFilter<'a>,
{
    type Item = Option<T::Item>;
    type ItemRef = Option<T::ItemRef>;

    fn get(entity: Entity, entries: &'a [QueryEntry]) -> Option<Self::ItemRef> {
        if let Some(item) = T::get(entity, entries) {
            Some(Some(item))
        } else {
            Some(None)
        }
    }

    fn maybes() -> Option<FxHashSet<u64>> {
        Some(FxHashSet::from_iter(vec![T::Item::component_id()]))
    }
}

/// Very similar to a Queryable, but instead of yielding a reference to the component, it is just used for filtering.
pub trait QueryFilter<'a> {
    fn withs() -> Option<FxHashSet<u64>> {
        None
    }
    fn withouts() -> Option<FxHashSet<u64>> {
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
    fn withs() -> Option<FxHashSet<u64>> {
        Some(FxHashSet::from_iter(vec![T::component_id()]))
    }
}

pub struct Without<'a, T>(std::marker::PhantomData<&'a T>)
where
    T: Component;

impl<'a, T> QueryFilter<'a> for Without<'a, T>
where
    T: Component,
{
    fn withouts() -> Option<FxHashSet<u64>> {
        Some(FxHashSet::from_iter(vec![T::component_id()]))
    }
}

pub struct Query<'a, T, F = ()>
where
    T: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub(crate) entries: FxHashMap<Entity, Vec<QueryEntry>>,

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

    pub fn get(&'a self, entity: Entity) -> Option<T::ItemRef> {
        self.entries
            .get(&entity)
            .and_then(|entries| T::get(entity, entries))
    }

    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entries.keys().copied()
    }

    pub fn iter(&'a self) -> Box<dyn Iterator<Item = T::ItemRef> + '_> {
        Box::new(
            self.entries
                .iter()
                .filter_map(move |(&entity, entries)| T::get(entity, entries)),
        )
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
            fn withs() -> Option<FxHashSet<u64>> {
                let mut all = FxHashSet::default();
                $(
                    if let Some(mut withs) = $name::withs() {
                        all.extend(withs.drain());
                    }
                )*
                if all.is_empty() {
                    return None;
                }
                Some(all)
            }

            fn withouts() -> Option<FxHashSet<u64>> {
                let mut all = FxHashSet::default();
                $(
                    if let Some(mut withouts) = $name::withouts() {
                        all.extend(withouts.drain());
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
