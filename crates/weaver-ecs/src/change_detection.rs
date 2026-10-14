//! Change ticks: the logical clock behind `Changed`, `is_added`/`is_changed`, and removal tracking.
//!
//! The world has a [`Tick`] counter that is bumped every time a system runs, so each run gets its
//! own `this_run`. Whenever a component or resource is added or written, its [`ComponentTicks`] get
//! stamped with the writer's `this_run`. A reader compares those stamps against a [`TickRange`]:
//! anything stamped after the reader's `last_run` (its own previous `this_run`) counts as new. That
//! makes change detection relative to the reader, so a system sees everything other systems did
//! since it last ran, whatever order they ran in.

use crate::{
    system::{SystemAccess, SystemParam},
    world::World,
};

/// A point on the world's logical change clock.
///
/// Ticks are plain `u64` counters. Comparisons go through
/// [`is_newer_than`](Tick::is_newer_than), which works on ages relative to the current tick and
/// so survives the counter wrapping.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub(crate) u64);

impl Tick {
    /// The largest tick value.
    pub const MAX: Self = Self(u64::MAX);

    /// Makes a tick from a raw counter value.
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the raw counter value.
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    /// Returns true if this tick falls after `last_run`, as seen from `this_run`.
    ///
    /// A tick equal to `last_run` is not newer. With `this_run` as the reference point, the answer
    /// is correct even when the counter has wrapped between the two.
    pub fn is_newer_than(&self, last_run: Tick, this_run: Tick) -> bool {
        let last_diff = this_run.relative_to(last_run).as_u64();
        let this_diff = this_run.relative_to(*self).as_u64();

        this_diff < last_diff
    }

    /// Returns how far this tick is ahead of `other`, wrapping on overflow.
    pub fn relative_to(&self, other: Tick) -> Tick {
        Tick(self.0.wrapping_sub(other.0))
    }

    /// How far back a system that has never run looks for changes. Anything stamped within this
    /// many ticks of its first run counts as added/changed on that run.
    pub const MAX_AGE: u64 = 1 << 62;

    /// The `last_run` tick for a system that has never run, relative to its first `this_run`.
    pub fn never_run(this_run: Tick) -> Tick {
        Tick(this_run.0.wrapping_sub(Self::MAX_AGE))
    }
}

/// The change ticks a system (or query) compares component ticks against: changes stamped after
/// `last_run` and up to `this_run` count as changed.
#[derive(Debug, Clone, Copy)]
pub struct TickRange {
    /// The reader's previous `this_run`. Changes stamped after this are new.
    pub last_run: Tick,
    /// The tick of the current run; also what writes made during it get stamped with.
    pub this_run: Tick,
}

impl TickRange {
    /// Makes a range from the reader's previous and current run ticks.
    pub fn new(last_run: Tick, this_run: Tick) -> Self {
        Self { last_run, this_run }
    }

    /// Ticks for a first run: everything that exists counts as added and changed.
    pub fn first_run(this_run: Tick) -> Self {
        Self {
            last_run: Tick::never_run(this_run),
            this_run,
        }
    }
}

/// The change ticks of the system that takes this parameter.
pub struct WorldTicks {
    /// The system's previous `this_run`.
    pub last_change_tick: Tick,
    /// The current run's tick.
    pub change_tick: Tick,
}

impl SystemParam for WorldTicks {
    type Item = WorldTicks;
    type State = ();

    fn access() -> SystemAccess {
        SystemAccess::default()
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(_world: &World, _state: &Self::State, ticks: TickRange) -> Self {
        WorldTicks {
            last_change_tick: ticks.last_run,
            change_tick: ticks.this_run,
        }
    }
}

/// When a single component value was added and last changed.
///
/// Stored per row next to the component data, and per resource.
#[derive(Debug, Clone, Copy, Default)]
pub struct ComponentTicks {
    /// The tick the value was added at.
    pub added: Tick,
    /// The tick the value was last written at. Equal to `added` for a value never written since.
    pub changed: Tick,
}

impl ComponentTicks {
    /// Ticks for a value created at `tick`: added and changed at the same moment.
    pub fn new(tick: Tick) -> Self {
        Self {
            added: tick,
            changed: tick,
        }
    }

    /// Stamps the value as changed at `tick`.
    pub fn set_changed(&mut self, tick: Tick) {
        self.changed = tick;
    }

    /// Returns true if the value was added after `last_run`, as seen from `this_run`.
    pub fn is_added(&self, last_run: Tick, this_run: Tick) -> bool {
        self.added.is_newer_than(last_run, this_run)
    }

    /// Returns true if the value was changed (or added) after `last_run`, as seen from `this_run`.
    pub fn is_changed(&self, last_run: Tick, this_run: Tick) -> bool {
        self.changed.is_newer_than(last_run, this_run)
    }
}

/// Read-only change information on a component or resource reference.
///
/// Implemented by the reference wrappers ([`Ref`](crate::storage::Ref),
/// [`Mut`](crate::storage::Mut), `Res`, `ResMut`) that carry the value's ticks and the reader's
/// [`TickRange`].
pub trait ChangeDetection {
    /// Returns true if the value was added since the reader last ran.
    fn is_added(&self) -> bool;
    /// Returns true if the value was added or changed since the reader last ran.
    fn is_changed(&self) -> bool;
    /// Returns the tick the value was last changed at.
    fn last_changed(&self) -> Tick;
}

/// Change information on a mutable reference, plus ways to write without (or only conditionally)
/// marking the value changed.
///
/// Going through `DerefMut` marks the value changed on every access; the methods here are for
/// when that would report false positives.
pub trait ChangeDetectionMut: ChangeDetection {
    /// The wrapped value's type.
    type Inner: ?Sized;
    /// Marks the value changed at the current run's tick.
    fn set_changed(&mut self);
    /// Sets the tick the value was last changed at.
    fn set_last_changed(&mut self, tick: Tick);
    /// Returns the value without marking it changed.
    fn bypass_change_detection(&mut self) -> &mut Self::Inner;

    /// Stores `value` and marks the value changed, but only if it differs from the current one.
    /// Returns true if it did.
    fn set_if_neq(&mut self, value: Self::Inner) -> bool
    where
        Self::Inner: PartialEq + Sized,
    {
        let old = self.bypass_change_detection();
        if *old != value {
            *old = value;
            self.set_changed();
            true
        } else {
            false
        }
    }

    /// Like [`set_if_neq`](ChangeDetectionMut::set_if_neq), but returns the old value when it was
    /// replaced, or `value` itself when it was equal and nothing happened.
    fn replace_if_neq(&mut self, value: Self::Inner) -> Self::Inner
    where
        Self::Inner: PartialEq + Sized,
    {
        let old = self.bypass_change_detection();
        if *old != value {
            let old = std::mem::replace(old, value);
            self.set_changed();
            old
        } else {
            value
        }
    }
}
