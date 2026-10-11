//! Run conditions: predicates that decide whether a system runs this time.
//!
//! Attach them with [`IntoSystemConfig::run_if`](crate::system::IntoSystemConfig::run_if). A
//! system whose conditions don't all hold is skipped without counting as a failure, so gating a
//! system on, say, a resource existing should be written as a condition rather than left to the
//! silent skip of a missing `Res<T>`.

use crate::{
    change_detection::{ChangeDetection, TickRange},
    component::Component,
    query::With,
    world::World,
};

/// A predicate evaluated before its system runs.
///
/// `ticks.last_run` is the owning system's last run (so "changed" means "changed since the system
/// last ran"), and `ticks.this_run` is the world's current tick.
pub trait Condition: Send + Sync + 'static {
    fn evaluate(&mut self, world: &World, ticks: TickRange) -> bool;

    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

impl<F> Condition for F
where
    F: FnMut(&World, TickRange) -> bool + Send + Sync + 'static,
{
    fn evaluate(&mut self, world: &World, ticks: TickRange) -> bool {
        self(world, ticks)
    }
}

pub type BoxedCondition = Box<dyn Condition>;

/// True if the resource exists.
pub fn resource_exists<T: Component>() -> impl Condition {
    |world: &World, _: TickRange| world.has_resource::<T>()
}

/// True if the resource exists and was added since the system last ran.
pub fn resource_added<T: Component>() -> impl Condition {
    |world: &World, ticks: TickRange| {
        world
            .get_resource_with_ticks::<T>(ticks)
            .is_some_and(|res| res.is_added())
    }
}

/// True if the resource exists and was added or changed since the system last ran.
pub fn resource_changed<T: Component>() -> impl Condition {
    |world: &World, ticks: TickRange| {
        world
            .get_resource_with_ticks::<T>(ticks)
            .is_some_and(|res| res.is_changed())
    }
}

/// True if at least one entity has a `T` component.
pub fn any_with_component<T: Component>() -> impl Condition {
    |world: &World, _: TickRange| world.query::<With<T>>().iter().next().is_some()
}

/// Inverts a condition.
pub fn not<C: Condition>(mut condition: C) -> impl Condition {
    move |world: &World, ticks: TickRange| !condition.evaluate(world, ticks)
}
