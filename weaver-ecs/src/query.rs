use std::sync::{RwLockReadGuard, RwLockWriteGuard};

use rustc_hash::FxHashMap;
use weaver_proc_macro::impl_queryable_for_n_tuple;

use crate::{component::Component, entity::Entity, World};

pub trait Queryable<'world, 'query, 'item>
where
    'world: 'query,
    'query: 'item,
{
    type Item: 'item;
    type ItemRef: 'item;

    fn can_write() -> bool
    where
        Self: Sized;

    fn construct(world: &'world World) -> Self
    where
        Self: Sized;

    fn get(&'query mut self, entity: Entity) -> Option<Self::ItemRef>;
    fn iter(&'query mut self) -> Box<dyn Iterator<Item = Self::ItemRef> + 'item>;
}

/// A `read` query. This is used to query for immutable references to components of a specific type.
pub struct Read<'a, T>
where
    T: Component,
{
    entries: FxHashMap<Entity, RwLockReadGuard<'a, dyn Component>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T> Read<'a, T>
where
    T: Component,
{
    /// Creates a new `read` query containing immutable references to components in the given `World`.
    pub(crate) fn new(world: &'a World) -> Self {
        let entries = world
            .entities_components
            .iter()
            .filter_map(|(entity, components)| {
                components
                    .iter()
                    .find(|component| component.read().unwrap().as_any().is::<T>())
                    .map(|component| (*entity, component.read().unwrap()))
            })
            .collect();

        Self {
            entries,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'world, 'query, 'item, T> Queryable<'world, 'query, 'item> for Read<'world, T>
where
    'world: 'query,
    'query: 'item,
    T: Component,
{
    type Item = T;
    type ItemRef = &'item T;

    fn can_write() -> bool {
        false
    }

    fn construct(world: &'world World) -> Self
    where
        Self: Sized,
    {
        Self::new(world)
    }

    /// Gets an immutable reference to the component for the given `Entity`.
    fn get(&'query mut self, entity: Entity) -> Option<&'item T> {
        self.entries
            .get(&entity)
            .map(|entry| &**entry)
            .map(|entry| entry.as_any().downcast_ref::<T>().unwrap())
    }

    /// Returns an iterator over the components in the query. This iterator will yield immutable references.
    fn iter(&'query mut self) -> Box<dyn Iterator<Item = &'item T> + '_> {
        Box::new(self.entries.values().map(|entry| &**entry).map(|entry| {
            entry
                .as_any()
                .downcast_ref::<T>()
                .expect("failed to downcast component")
        }))
    }
}

/// A `write` query. This is used to query for mutable references to components of a specific type.
pub struct Write<'a, T>
where
    T: Component,
{
    entries: FxHashMap<Entity, RwLockWriteGuard<'a, dyn Component>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T> Write<'a, T>
where
    T: Component,
{
    /// Creates a new `write` query containing mutable references to components in the given `World`.
    pub(crate) fn new(world: &'a World) -> Self {
        let entries = world
            .entities_components
            .iter()
            .filter_map(|(entity, components)| {
                components
                    .iter()
                    .find(|component| component.read().unwrap().as_any().is::<T>())
                    .map(|component| (*entity, component.write().unwrap()))
            })
            .collect();

        Self {
            entries,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'world, 'query, 'item, T> Queryable<'world, 'query, 'item> for Write<'world, T>
where
    'world: 'query,
    'query: 'item,
    T: Component,
{
    type Item = T;
    type ItemRef = &'item mut T;

    fn can_write() -> bool {
        true
    }

    fn construct(world: &'world World) -> Self
    where
        Self: Sized,
    {
        Self::new(world)
    }

    /// Gets a mutable reference to the component for the given `Entity`.
    fn get(&'query mut self, entity: Entity) -> Option<Self::ItemRef> {
        self.entries
            .get_mut(&entity)
            .map(|entry| &mut **entry)
            .map(|entry| entry.as_any_mut().downcast_mut::<T>().unwrap())
    }

    /// Returns an iterator over the components in the query. This iterator will yield mutable references.
    fn iter(&'query mut self) -> Box<dyn Iterator<Item = Self::ItemRef> + 'item> {
        Box::new(
            self.entries
                .values_mut()
                .map(|entry| &mut **entry)
                .map(|entry| {
                    entry
                        .as_any_mut()
                        .downcast_mut::<T>()
                        .expect("failed to downcast component")
                }),
        )
    }
}

impl_queryable_for_n_tuple!(2);
// impl_queryable_for_n_tuple!(3);
// impl_queryable_for_n_tuple!(4);

/// A query that can be used to request references to components from a `World`.
///
/// By default, only entities that contain all components in the query will be returned.
/// Include a `Without` query in the type parameter to request entities that do not contain the component.
/// Include a `Maybe` query in the type parameter to request entities that may or may not contain the component.
pub struct Query<'world, 'query, 'item, Q>
where
    'world: 'query,
    'query: 'item,
    Q: Queryable<'world, 'query, 'item>,
{
    inner: Q,
    _phantom: std::marker::PhantomData<(&'world (), &'query (), &'item ())>,
}

impl<'world, 'query, 'item, Q> Query<'world, 'query, 'item, Q>
where
    'world: 'query,
    'query: 'item,
    Q: Queryable<'world, 'query, 'item>,
{
    /// Creates a new query containing references to components in the given `World`.
    pub(crate) fn new(world: &'world World) -> Self {
        Self {
            inner: Q::construct(world),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Gets a reference to the component for the given `Entity`.
    pub fn get(&'query mut self, entity: Entity) -> Option<Q::ItemRef> {
        self.inner.get(entity)
    }

    /// Returns an iterator over the components in the query.
    pub fn iter(&'query mut self) -> Box<dyn Iterator<Item = Q::ItemRef> + 'item> {
        self.inner.iter()
    }
}
