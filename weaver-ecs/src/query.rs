use std::sync::RwLockReadGuard;

use rustc_hash::FxHashMap;

use crate::{component::Component, entity::Entity};

/// A `read` query. Also acts as a guard holding a borrow on the world.
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
    pub fn get(&self, entity: Entity) -> Option<&T> {
        self.entries
            .get(&entity)
            .map(|entry| &**entry)
            .map(|entry| entry.as_any().downcast_ref::<T>().unwrap())
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        self.entries.values().map(|entry| &**entry).map(|entry| {
            entry
                .as_any()
                .downcast_ref::<T>()
                .expect("failed to downcast component")
        })
    }

    // fn iter(&self) -> Box<dyn Iterator<Item = &T> + '_> {
    //     Box::new(self.entries.values().map(|entry| &**entry)).map(|entry| {
    //         entry
    //             .as_any()
    //             .downcast_ref::<T>()
    //             .expect("failed to downcast component")
    //     })
    // }
}

impl<'a, T> Read<'a, T>
where
    T: Component,
{
    pub fn new(world: &'a crate::World) -> Self {
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
