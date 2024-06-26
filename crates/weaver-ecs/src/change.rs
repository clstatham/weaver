use weaver_util::lock::{Read, Write};

use crate::prelude::{SystemAccess, SystemParam, UnsafeWorldCell};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick {
    tick: u64,
}

impl Tick {
    pub const MAX: Self = Self { tick: u64::MAX };

    pub fn new(tick: u64) -> Self {
        Self { tick }
    }

    pub fn get(&self) -> u64 {
        self.tick
    }

    pub fn set(&mut self, tick: u64) {
        self.tick = tick;
    }

    pub fn is_newer_than(&self, last_run: Tick, this_run: Tick) -> bool {
        let last_diff = this_run.relative_to(last_run).tick;
        let this_diff = this_run.relative_to(*self).tick;

        this_diff < last_diff
    }

    pub fn relative_to(&self, other: Tick) -> Self {
        Self {
            tick: self.tick.wrapping_sub(other.tick),
        }
    }
}

impl std::fmt::Display for Tick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.tick)
    }
}

pub(crate) struct Ticks<'a> {
    pub(crate) added: Read<'a, Tick>,
    pub(crate) changed: Read<'a, Tick>,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

pub(crate) struct TicksMut<'a> {
    pub(crate) added: Write<'a, Tick>,
    pub(crate) changed: Write<'a, Tick>,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

pub trait ChangeDetection {
    fn is_added(&self) -> bool;
    fn is_changed(&self) -> bool;
    fn last_changed(&self) -> Tick;
}

pub trait ChangeDetectionMut: ChangeDetection {
    type Inner: ?Sized;
    fn set_changed(&mut self);
    fn bypass_change_detection(&mut self) -> &mut Self::Inner;

    fn set_if_neq(&mut self, other: Self::Inner) -> bool
    where
        Self::Inner: Sized + PartialEq,
    {
        let old = self.bypass_change_detection();
        if *old != other {
            *old = other;
            self.set_changed();
            true
        } else {
            false
        }
    }

    fn replace_if_neq(&mut self, other: Self::Inner) -> Option<Self::Inner>
    where
        Self::Inner: Sized + PartialEq,
    {
        let old = self.bypass_change_detection();
        if *old != other {
            let old = std::mem::replace(old, other);
            self.set_changed();
            Some(old)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentTicks {
    pub(crate) added: Tick,
    pub(crate) changed: Tick,
}

impl ComponentTicks {
    pub fn new(changed: Tick) -> Self {
        Self {
            added: changed,
            changed,
        }
    }

    pub fn is_added(&self, last_run: Tick, this_run: Tick) -> bool {
        self.added.is_newer_than(last_run, this_run)
    }

    pub fn is_changed(&self, last_run: Tick, this_run: Tick) -> bool {
        self.changed.is_newer_than(last_run, this_run)
    }

    pub fn set_changed(&mut self, tick: Tick) {
        self.changed = tick;
    }
}

pub struct WorldTicks {
    pub change_tick: Tick,
    pub last_change_tick: Tick,
}

unsafe impl SystemParam for WorldTicks {
    type State = ();
    type Item<'w, 's> = Self;

    fn validate_access(_access: &SystemAccess) -> bool {
        true
    }

    fn access() -> crate::prelude::SystemAccess {
        SystemAccess {
            exclusive: false,
            ..Default::default()
        }
    }

    fn init_state(_world: &mut crate::prelude::World) -> Self::State {}

    unsafe fn fetch<'w, 's>(
        _state: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        Self {
            change_tick: world.read_change_tick(),
            last_change_tick: world.last_change_tick(),
        }
    }
}
