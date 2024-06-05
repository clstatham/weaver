#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Entity {
    id: u32,
    generation: u32,
}

impl Entity {
    pub const fn new(id: u32, generation: u32) -> Self {
        Self { id, generation }
    }

    pub const fn id(&self) -> u32 {
        self.id
    }

    pub const fn generation(&self) -> u32 {
        self.generation
    }

    pub const fn as_u64(&self) -> u64 {
        (self.generation as u64) << 32 | self.id as u64
    }

    pub const fn from_u64(value: u64) -> Self {
        Self {
            id: value as u32,
            generation: (value >> 32) as u32,
        }
    }

    pub const fn as_usize(&self) -> usize {
        self.as_u64() as usize
    }

    pub const fn from_usize(value: usize) -> Self {
        Self::from_u64(value as u64)
    }
}
