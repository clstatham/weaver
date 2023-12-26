use std::sync::{RwLockReadGuard, RwLockWriteGuard};

use rustc_hash::FxHashMap;

use crate::{component::Component, entity::Entity, World};

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

    /// Gets an immutable reference to the component of type `T` for the given `Entity`.
    pub fn get(&self, entity: Entity) -> Option<&T> {
        self.entries
            .get(&entity)
            .map(|entry| &**entry)
            .map(|entry| entry.as_any().downcast_ref::<T>().unwrap())
    }

    /// Returns an iterator over the components of type `T` in the query.
    pub fn iter(&self) -> Box<dyn Iterator<Item = &T> + '_> {
        Box::new(self.entries.values().map(|entry| &**entry).map(|entry| {
            entry
                .as_any()
                .downcast_ref::<T>()
                .expect("failed to downcast component")
        }))
    }
}

/// A `write` query. This is used to query for mutable references to components of a specific type.
pub struct Write<'a, T: Component> {
    entries: FxHashMap<Entity, RwLockWriteGuard<'a, dyn Component>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, 'b: 'a, T: Component> Write<'a, T> {
    /// Creates a new `write` query containing mutable references to components of type `T` in the given `World`.
    pub(crate) fn new(world: &'b World) -> Self {
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

    /// Gets an immutable reference to the component of type `T` for the given `Entity`.
    pub fn get(&self, entity: Entity) -> Option<&T> {
        self.entries
            .get(&entity)
            .map(|entry| &**entry)
            .map(|entry| entry.as_any().downcast_ref::<T>().unwrap())
    }

    /// Gets a mutable reference to the component of type `T` for the given `Entity`.
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        self.entries
            .get_mut(&entity)
            .map(|entry| &mut **entry)
            .map(|entry| entry.as_any_mut().downcast_mut::<T>().unwrap())
    }

    /// Returns an iterator over the components of type `T` in the query. This iterator will yield immutable references.
    pub fn iter(&self) -> Box<dyn Iterator<Item = &T> + '_> {
        Box::new(self.entries.values().map(|entry| &**entry).map(|entry| {
            entry
                .as_any()
                .downcast_ref::<T>()
                .expect("failed to downcast component")
        }))
    }

    /// Returns an iterator over the components of type `T` in the query. This iterator will yield mutable references.
    pub fn iter_mut(&mut self) -> Box<dyn Iterator<Item = &mut T> + '_> {
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
