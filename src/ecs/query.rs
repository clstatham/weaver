use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLockReadGuard, RwLockWriteGuard,
};

use super::{
    entity::EntityId,
    storage::{ComponentSet, Components, QueryMap, Set},
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

        Set::<usize>::set_union_with(&mut includes, &self.reads);
        Set::<usize>::set_union_with(&mut includes, &self.writes);
        Set::<usize>::set_union_with(&mut includes, &self.withs);

        let mut filtered = archetype.clone();

        Set::<usize>::set_difference_with(&mut filtered, &self.withouts);
        Set::<usize>::set_intersection_with(&mut filtered, &includes);

        Set::<usize>::set_eq(&filtered, &includes)
    }
}

pub trait Queryable<'a, F = ()>
where
    F: QueryFilter<'a>,
{
    type Item: Bundle;
    type ItemRef: 'a + Send;

    // Gets the item from the given components, if it exists.
    fn get(entries: &'a [ComponentPtr]) -> Option<Self::ItemRef>;

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
fn collect_components<'a, T: Queryable<'a, F>, F: QueryFilter<'a>>(
    components: &Components,
) -> QueryMap {
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
    Set::<usize>::set_union_with(&mut required, &access.reads);
    Set::<usize>::set_union_with(&mut required, &access.writes);

    let mut entries = QueryMap::default();

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

    for entities in matching_archetypes {
        for entity in entities.set_iter() {
            let mut entity_entries = Vec::with_capacity(Set::<usize>::set_len(&required));
            for component_id in required.set_iter() {
                let component = components
                    .entity_components
                    .get(entity)
                    .and_then(|components| components.get(component_id));

                if let Some(component) = component {
                    entity_entries.push(component.clone());
                } else {
                    break;
                }
            }
            if entity_entries.len() == Set::<usize>::set_len(&required) {
                if let Some(entry) = entries.get_mut(entity) {
                    entry.extend(entity_entries);
                } else {
                    entries.insert(entity, entity_entries);
                }
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

    fn get(entries: &'a [ComponentPtr]) -> Option<Self::ItemRef> {
        entries.iter().find_map(|entry| {
            if entry.component_id == T::static_id() {
                Some(RwLockReadGuard::map(entry.component.read(), |component| {
                    component.as_any().downcast_ref::<T>().unwrap()
                }))
            } else {
                None
            }
        })
    }

    fn reads() -> Option<ComponentSet> {
        Some(ComponentSet::set_from_iter(vec![T::static_id()]))
    }
}

impl<'a, T, F> Queryable<'a, F> for &'a mut T
where
    T: Component,
    F: QueryFilter<'a>,
{
    type Item = T;
    type ItemRef = MappedRwLockWriteGuard<'a, T>;

    fn get(entries: &'a [ComponentPtr]) -> Option<Self::ItemRef> {
        entries.iter().find_map(|entry| {
            if entry.component_id == T::static_id() {
                Some(RwLockWriteGuard::map(
                    entry.component.write(),
                    |component| component.as_any_mut().downcast_mut::<T>().unwrap(),
                ))
            } else {
                None
            }
        })
    }

    fn writes() -> Option<ComponentSet> {
        Some(ComponentSet::set_from_iter(vec![T::static_id()]))
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
        Some(ComponentSet::set_from_iter(vec![T::static_id()]))
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
        Some(ComponentSet::set_from_iter(vec![T::static_id()]))
    }
}

pub struct Query<'a, T, F = ()>
where
    T: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub(crate) entries: QueryMap,

    _phantom: std::marker::PhantomData<&'a (T, F)>,
}

impl<'a, T, F> Query<'a, T, F>
where
    T: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    pub(crate) fn new(world: &World) -> Self {
        let entries = collect_components::<T, F>(&world.components.read());

        Self {
            entries,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn get(&'a self, entity: EntityId) -> Option<T::ItemRef> {
        self.entries.get(entity).and_then(|entries| T::get(entries))
    }

    pub fn entities(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.entries.keys()
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
                        Set::<usize>::set_union_with(&mut all, &withs);
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
                        Set::<usize>::set_union_with(&mut all, &withouts);
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
