use std::hash::{BuildHasherDefault, Hash, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

impl Hash for Entity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.as_u64());
    }
}

#[derive(Debug, Default)]
pub struct EntityHasher {
    state: u64,
}

impl Hasher for EntityHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, _: &[u8]) {
        panic!("EntityHasher does not support writing bytes");
    }

    fn write_u64(&mut self, i: u64) {
        // See the Bevy source code for this one (bevy-ecs/src/entity/hash.rs)
        const UPPER_PHI: u64 = 0x9e37_79b9_0000_0001;
        self.state = i.wrapping_mul(UPPER_PHI);
    }
}

pub type EntityMap<V> =
    std::collections::hash_map::HashMap<Entity, V, BuildHasherDefault<EntityHasher>>;

pub type EntitySet = std::collections::hash_set::HashSet<Entity, BuildHasherDefault<EntityHasher>>;
