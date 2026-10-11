use crate::{
    system::{SystemAccess, SystemParam},
    world::World,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub(crate) u64);

impl Tick {
    pub const MAX: Self = Self(u64::MAX);

    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn is_newer_than(&self, last_run: Tick, this_run: Tick) -> bool {
        let last_diff = this_run.relative_to(last_run).as_u64();
        let this_diff = this_run.relative_to(*self).as_u64();

        this_diff < last_diff
    }

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
    pub last_run: Tick,
    pub this_run: Tick,
}

impl TickRange {
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
    pub last_change_tick: Tick,
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

#[derive(Debug, Clone, Copy, Default)]
pub struct ComponentTicks {
    pub added: Tick,
    pub changed: Tick,
}

impl ComponentTicks {
    pub fn new(tick: Tick) -> Self {
        Self {
            added: tick,
            changed: tick,
        }
    }

    pub fn set_changed(&mut self, tick: Tick) {
        self.changed = tick;
    }

    pub fn is_added(&self, last_run: Tick, this_run: Tick) -> bool {
        self.added.is_newer_than(last_run, this_run)
    }

    pub fn is_changed(&self, last_run: Tick, this_run: Tick) -> bool {
        self.changed.is_newer_than(last_run, this_run)
    }
}

pub trait ChangeDetection {
    fn is_added(&self) -> bool;
    fn is_changed(&self) -> bool;
    fn last_changed(&self) -> Tick;
}

pub trait ChangeDetectionMut: ChangeDetection {
    type Inner: ?Sized;
    fn set_changed(&mut self);
    fn set_last_changed(&mut self, tick: Tick);
    fn bypass_change_detection(&mut self) -> &mut Self::Inner;

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
