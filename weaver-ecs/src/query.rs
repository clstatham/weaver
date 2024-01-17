use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLockReadGuard, RwLockWriteGuard,
};

use crate::{
    archetype::Archetype,
    storage::{Components, EntitySet},
};

use super::{entity::EntityId, storage::ComponentSet, Bundle, Component};

pub struct Ref<'a, T>
where
    T: Component,
{
    pub entity: EntityId,
    component: MappedRwLockReadGuard<'a, T>,
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
    pub entity: EntityId,
    component: MappedRwLockWriteGuard<'a, T>,
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

#[derive(Clone, Default)]
pub struct QueryAccess {
    pub reads: ComponentSet,
    pub writes: ComponentSet,
    pub withs: ComponentSet,
    pub withouts: ComponentSet,
}

impl QueryAccess {
    pub fn matches_archetype(&self, archetype: &Archetype) -> bool {
        let mut includes = ComponentSet::default();

        includes.extend(&self.reads);
        includes.extend(&self.writes);
        includes.extend(&self.withs);

        if !self.withouts.is_empty() && self.withouts.is_subset(&archetype.components) {
            return false;
        }

        includes.is_subset(&archetype.components)
    }

    pub fn fetched_components(&self) -> ComponentSet {
        let mut fetched = ComponentSet::default();

        fetched.extend(&self.reads);
        fetched.extend(&self.writes);

        fetched
    }
}

pub trait Queryable<'a, F = ()>
where
    F: QueryFilter<'a>,
{
    type Item: Bundle;
    type ItemRef: 'a + Send;

    fn get(entity: EntityId, components: &'a Components) -> Option<Self::ItemRef>;

    fn access() -> QueryAccess;
}

impl<'a, F> Queryable<'a, F> for ()
where
    F: QueryFilter<'a>,
{
    type Item = ();
    type ItemRef = ();

    fn get(_entity: EntityId, _components: &'a Components) -> Option<Self::ItemRef> {
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

    fn get(entity: EntityId, components: &'a Components) -> Option<Self::ItemRef> {
        let components = components.entity_components.get(&entity)?;
        let component = components.get(&T::static_id())?;
        let component = RwLockReadGuard::map(component.component.read(), |component| {
            (*component).as_any().downcast_ref::<T>().unwrap()
        });
        Some(Ref {
            entity,
            component,
            _marker: std::marker::PhantomData,
        })
    }

    fn access() -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::from_iter([T::static_id()]),
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
    fn get(entity: EntityId, components: &'a Components) -> Option<Self::ItemRef> {
        let components = components.entity_components.get(&entity)?;
        let component = components.get(&T::static_id()).map(|component| {
            RwLockWriteGuard::map(component.component.write(), |component| {
                (*component).as_any_mut().downcast_mut::<T>().unwrap()
            })
        })?;
        Some(Mut {
            entity,
            component,
            _marker: std::marker::PhantomData,
        })
    }

    fn access() -> QueryAccess {
        QueryAccess {
            reads: ComponentSet::default(),
            writes: ComponentSet::from_iter([T::static_id()]),
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
        ComponentSet::from_iter([T::static_id()])
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
        ComponentSet::from_iter([T::static_id()])
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
        Q::get(entity, components)
    }

    pub fn iter(&'a self, components: &'a Components) -> QueryIter<'a, Q, F> {
        QueryIter {
            components,
            entities: self.entities.iter(),
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
    components: &'a Components,
    entities: std::collections::hash_set::Iter<'a, EntityId>,
    _filter: std::marker::PhantomData<(Q, F)>,
}

impl<'a, Q, F> Iterator for QueryIter<'a, Q, F>
where
    Q: Queryable<'a, F>,
    F: QueryFilter<'a>,
{
    type Item = Q::ItemRef;

    fn next(&mut self) -> Option<Self::Item> {
        for entity in self.entities.by_ref() {
            if let Some(item) = Q::get(*entity, self.components) {
                return Some(item);
            }
        }

        None
    }
}

macro_rules! impl_queryable_for_tuple {
    ($($name:ident),*) => {
        impl<'a, $($name),*, F> Queryable<'a, F> for ($($name,)*)
        where
            $($name: Queryable<'a>,)*
            F: QueryFilter<'a>,
        {
            type Item = ($($name::Item,)*);
            type ItemRef = ($($name::ItemRef,)*);

            fn get(entity: EntityId, components: &'a Components) -> Option<Self::ItemRef> {
                Some(($($name::get(entity, components)?,)*))
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
