use std::sync::{RwLockReadGuard, RwLockWriteGuard};

use rustc_hash::FxHashMap;
use weaver_proc_macro::impl_queryable_for_n_tuple;

use crate::{component::Component, entity::Entity, World};

pub trait Queryable<'a, 'b>
where
    'a: 'b,
{
    type Item: 'b;
    type ItemRef: 'b;
    type ItemMut: 'b;

    fn can_write() -> bool
    where
        Self: Sized;

    fn construct(world: &'a World) -> Self
    where
        Self: Sized;

    fn get(&'a self, entity: Entity) -> Option<Self::ItemRef>;

    fn get_mut(&'a mut self, entity: Entity) -> Option<Self::ItemMut>;

    fn iter(&'a self) -> Box<dyn Iterator<Item = Self::ItemRef> + 'b>;

    fn iter_mut(&'a mut self) -> Box<dyn Iterator<Item = Self::ItemMut> + 'b>;
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
    /// Creates a new `read` query containing immutable references to components of type `T` in the given `World`.
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

impl<'a, 'b, T> Queryable<'a, 'b> for Read<'a, T>
where
    'a: 'b,
    T: Component,
{
    type Item = T;
    type ItemRef = &'b T;
    type ItemMut = &'b mut T;

    fn can_write() -> bool {
        false
    }

    fn construct(world: &'a World) -> Self
    where
        Self: Sized,
    {
        Self::new(world)
    }

    /// Gets an immutable reference to the component of type `C` for the given `Entity`.
    fn get(&'a self, entity: Entity) -> Option<&'b T> {
        self.entries
            .get(&entity)
            .map(|entry| &**entry)
            .map(|entry| entry.as_any().downcast_ref::<T>().unwrap())
    }

    /// Gets a mutable reference to the component of type `C` for the given `Entity`.
    ///
    /// This will always return `None` for a `read` query.
    fn get_mut(&mut self, entity: Entity) -> Option<&'b mut T> {
        None
    }

    /// Returns an iterator over the components of type `C` in the query. This iterator will yield immutable references.
    fn iter(&'a self) -> Box<dyn Iterator<Item = &'b T> + '_> {
        Box::new(self.entries.values().map(|entry| &**entry).map(|entry| {
            entry
                .as_any()
                .downcast_ref::<T>()
                .expect("failed to downcast component")
        }))
    }

    /// Returns an iterator over the components of type `C` in the query. This iterator will yield mutable references.
    ///
    /// # Panics
    /// This will ***ALWAYS*** panic for a `read` query. Do not use this method.
    fn iter_mut(&'a mut self) -> Box<dyn Iterator<Item = &'b mut T> + '_> {
        unimplemented!("cannot iterate over a read query mutably")
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
    /// Creates a new `write` query containing mutable references to components of type `T` in the given `World`.
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

impl<'a, 'b, T> Queryable<'a, 'b> for Write<'a, T>
where
    'a: 'b,
    T: Component,
{
    type Item = T;
    type ItemRef = &'b T;
    type ItemMut = &'b mut T;

    fn can_write() -> bool {
        true
    }

    fn construct(world: &'a World) -> Self
    where
        Self: Sized,
    {
        Self::new(world)
    }

    /// Gets an immutable reference to the component of type `C` for the given `Entity`.
    fn get(&'a self, entity: Entity) -> Option<&'b T> {
        self.entries
            .get(&entity)
            .map(|entry| &**entry)
            .map(|entry| entry.as_any().downcast_ref::<T>().unwrap())
    }

    /// Gets a mutable reference to the component of type `C` for the given `Entity`.
    fn get_mut(&'a mut self, entity: Entity) -> Option<&'b mut T> {
        self.entries
            .get_mut(&entity)
            .map(|entry| &mut **entry)
            .map(|entry| entry.as_any_mut().downcast_mut::<T>().unwrap())
    }

    /// Returns an iterator over the components of type `C` in the query. This iterator will yield immutable references.
    fn iter(&'a self) -> Box<dyn Iterator<Item = &'b T> + '_> {
        Box::new(self.entries.values().map(|entry| &**entry).map(|entry| {
            entry
                .as_any()
                .downcast_ref::<T>()
                .expect("failed to downcast component")
        }))
    }

    /// Returns an iterator over the components of type `C` in the query. This iterator will yield mutable references.
    fn iter_mut(&'a mut self) -> Box<dyn Iterator<Item = &'b mut T> + '_> {
        Box::new(
            self.entries
                .values_mut()
                .map(|entry| &mut **entry)
                .map(|entry| entry.as_any_mut().downcast_mut::<T>().unwrap()),
        )
    }
}

impl_queryable_for_n_tuple!(2);

/// A query that can be used to request references to components from a `World`.
///
/// By default, only entities that contain all components in the query will be returned.
/// Include a `Without` query in the type parameter to request entities that do not contain the component.
/// Include a `Maybe` query in the type parameter to request entities that may or may not contain the component.
pub struct Query<'a, 'b, Q>
where
    'a: 'b,
    Q: Queryable<'a, 'b>,
{
    inner: Q,
    _phantom: std::marker::PhantomData<(&'a (), &'b ())>,
}

impl<'a, 'b, Q: Queryable<'a, 'b>> Query<'a, 'b, Q>
where
    'a: 'b,
{
    /// Creates a new query from the given `Queryable`.
    pub(crate) fn from_inner(inner: Q) -> Self {
        Self {
            inner,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Creates a new query from the given `World`.
    pub(crate) fn new(world: &'a World) -> Self {
        Self::from_inner(Q::construct(world))
    }

    /// Gets an immutable reference to the component of type `C` for the given `Entity`.
    pub fn get(&'a self, entity: Entity) -> Option<Q::ItemRef> {
        self.inner.get(entity)
    }

    /// Gets a mutable reference to the component of type `C` for the given `Entity`.
    pub fn get_mut(&'a mut self, entity: Entity) -> Option<Q::ItemMut> {
        self.inner.get_mut(entity)
    }

    /// Returns an iterator over the components of type `C` in the query. This iterator will yield immutable references.
    pub fn iter(&'a self) -> Box<dyn Iterator<Item = Q::ItemRef> + 'b> {
        self.inner.iter()
    }

    /// Returns an iterator over the components of type `C` in the query. This iterator will yield mutable references.
    pub fn iter_mut(&'a mut self) -> Box<dyn Iterator<Item = Q::ItemMut> + 'b> {
        self.inner.iter_mut()
    }
}
