//! Removal tracking: which entities lost a component, for the [`RemovedComponents`] parameter.

use std::{any::TypeId, marker::PhantomData};

use weaver_util::prelude::*;

use crate::{
    change_detection::{Tick, TickRange},
    component::Component,
    entity::Entity,
    system::{SystemAccess, SystemParam},
    world::World,
};

/// Which entities lost which component types, and when.
///
/// Every removal is stamped with the change tick it happened at, so a reader only sees removals
/// newer than its own last run. Entries are kept for two [`World::clear_trackers`] calls (one
/// frame for a world that calls it once per frame), so a system that runs every frame never misses
/// one.
#[derive(Default)]
pub struct RemovedComponentsLog {
    logs: TypeIdMap<DoubleBuffer>,
}

#[derive(Default)]
struct DoubleBuffer {
    previous: Vec<(Entity, Tick)>,
    current: Vec<(Entity, Tick)>,
}

impl RemovedComponentsLog {
    pub(crate) fn record(&mut self, ty: TypeId, entity: Entity, tick: Tick) {
        self.logs
            .entry(ty)
            .or_default()
            .current
            .push((entity, tick));
    }

    /// Drops the older half of every log.
    pub(crate) fn clear(&mut self) {
        for log in self.logs.values_mut() {
            std::mem::swap(&mut log.previous, &mut log.current);
            log.current.clear();
        }
    }

    /// Entities that lost a `ty` component within `ticks`.
    pub fn removed(&self, ty: TypeId, ticks: TickRange) -> Vec<Entity> {
        let Some(log) = self.logs.get(&ty) else {
            return Vec::new();
        };
        log.previous
            .iter()
            .chain(&log.current)
            .filter(|(_, tick)| tick.is_newer_than(ticks.last_run, ticks.this_run))
            .map(|&(entity, _)| entity)
            .collect()
    }
}

/// The entities that had a `T` component removed (or were despawned while they had one) since this
/// system last ran.
///
/// Each removal is reported once per reading system. A removal is forgotten after two
/// [`World::clear_trackers`] calls, so a system that doesn't run for longer than that misses it.
pub struct RemovedComponents<T: Component> {
    entities: Vec<Entity>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Component> RemovedComponents<T> {
    /// Iterates over the entities.
    pub fn iter(&self) -> std::slice::Iter<'_, Entity> {
        self.entities.iter()
    }

    /// Returns how many removals there are.
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Returns true if nothing was removed.
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}

impl<'a, T: Component> IntoIterator for &'a RemovedComponents<T> {
    type Item = &'a Entity;
    type IntoIter = std::slice::Iter<'a, Entity>;

    fn into_iter(self) -> Self::IntoIter {
        self.entities.iter()
    }
}

impl<T: Component> IntoIterator for RemovedComponents<T> {
    type Item = Entity;
    type IntoIter = std::vec::IntoIter<Entity>;

    fn into_iter(self) -> Self::IntoIter {
        self.entities.into_iter()
    }
}

impl<T: Component> SystemParam for RemovedComponents<T> {
    type Item = RemovedComponents<T>;
    type State = ();

    fn access() -> SystemAccess {
        SystemAccess::default()
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _state: &Self::State, ticks: TickRange) -> Self::Item {
        RemovedComponents {
            entities: world.removed_components::<T>(ticks),
            _marker: PhantomData,
        }
    }
}
