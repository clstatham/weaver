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

    pub fn is_newer(&self, other: Tick) -> bool {
        self.tick > other.tick
    }

    pub fn relative_to(&self, other: Tick) -> u64 {
        self.tick - other.tick
    }
}

impl std::fmt::Display for Tick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.tick)
    }
}
