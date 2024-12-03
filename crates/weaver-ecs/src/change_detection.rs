use crate::{
    system::{SystemAccess, SystemParam},
    world::World,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub(crate) u64);

impl Tick {
    pub const MAX: Self = Self(u64::MAX);

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
}

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

    fn fetch(world: &World, _state: &Self::State) -> Self {
        WorldTicks {
            last_change_tick: world.last_change_tick(),
            change_tick: world.read_change_tick(),
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
