use std::{
    hash::{BuildHasherDefault, Hash, Hasher},
    num::NonZeroU32,
    sync::atomic::{AtomicI64, Ordering},
};

use weaver_util::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct Entity {
    id: u32,
    generation: NonZeroU32,
}

impl Entity {
    pub const fn new(id: u32, generation: NonZeroU32) -> Self {
        Self { id, generation }
    }

    pub const fn id(&self) -> u32 {
        self.id
    }

    pub const fn generation(&self) -> u32 {
        self.generation.get()
    }

    pub const fn as_u64(&self) -> u64 {
        ((self.generation() as u64) << 32) | self.id as u64
    }

    pub fn from_u64(value: u64) -> Self {
        Self {
            id: value as u32,
            generation: NonZeroU32::new((value >> 32) as u32).unwrap(),
        }
    }

    pub const fn as_usize(&self) -> usize {
        self.as_u64() as usize
    }

    pub fn from_usize(value: usize) -> Self {
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

    fn write_u32(&mut self, i: u32) {
        self.write_u64(i as u64);
    }
}

pub type EntityMap<V> =
    std::collections::hash_map::HashMap<Entity, V, BuildHasherDefault<EntityHasher>>;

pub type EntitySet = std::collections::hash_set::HashSet<Entity, BuildHasherDefault<EntityHasher>>;

#[derive(Debug, Default)]
pub struct Entities {
    // Weaver uses a system similar to Bevy's entity system to manage entities.
    free_cursor: AtomicI64,
    pending: Vec<u32>,
    generations: Vec<NonZeroU32>,
}

impl Entities {
    pub fn needs_flush(&mut self) -> bool {
        *self.free_cursor.get_mut() != self.pending.len() as i64
    }

    pub fn verify_flushed(&mut self) {
        assert!(
            !self.needs_flush(),
            "Entities::flush() must be called before this operation"
        );
    }

    pub fn flush(&mut self) {
        let free_cursor = self.free_cursor.get_mut();
        let current_free_cursor = *free_cursor;

        let new_free_cursor = if current_free_cursor < 0 {
            let old_len = self.generations.len();
            let new_len = old_len + -current_free_cursor as usize;
            self.generations.resize(new_len, NonZeroU32::MIN);
            *free_cursor = 0;
            0
        } else {
            current_free_cursor as usize
        };

        self.pending.drain(new_free_cursor..);
    }

    pub fn reserve(&self) -> Entity {
        let n = self.free_cursor.fetch_sub(1, Ordering::Relaxed);
        if n > 0 {
            let index = self.pending[n as usize - 1];
            Entity::new(index, self.generations[index as usize])
        } else {
            let index = self.generations.len() as i64 - n;
            Entity::new(u32::try_from(index).unwrap(), NonZeroU32::MIN)
        }
    }

    pub fn alloc(&mut self) -> Entity {
        self.verify_flushed();

        if let Some(index) = self.pending.pop() {
            let new_free_cursor = self.pending.len() as i64;
            *self.free_cursor.get_mut() = new_free_cursor;
            Entity::new(index, self.generations[index as usize])
        } else {
            let index = u32::try_from(self.generations.len()).unwrap();
            self.generations.push(NonZeroU32::MIN);
            Entity::new(index, NonZeroU32::MIN)
        }
    }

    pub fn alloc_at(&mut self, entity: Entity) {
        self.verify_flushed();

        let id = entity.id as usize;
        if id >= self.generations.len() {
            self.generations.resize(id + 1, NonZeroU32::MIN);
        } else if let Some(index) = self.pending.iter().position(|&i| i == entity.id) {
            self.pending.swap_remove(index);
            let new_free_cursor = self.pending.len() as i64;
            *self.free_cursor.get_mut() = new_free_cursor;
        }

        self.generations[id] = entity.generation;
    }

    pub fn free(&mut self, entity: Entity) {
        self.verify_flushed();

        let generation = &mut self.generations[entity.id as usize];
        if generation.get() != entity.generation() {
            // entity is already dead
            return;
        }

        *generation = NonZeroU32::new(generation.get().wrapping_add(1)).unwrap();

        if *generation == NonZeroU32::MIN {
            log::warn!("Entity generation wrapped around on entity {:?}", entity.id);
        }

        self.pending.push(entity.id);

        let new_free_cursor = self.pending.len() as i64;
        *self.free_cursor.get_mut() = new_free_cursor;
    }

    pub fn find_by_id(&self, id: u32) -> Option<Entity> {
        let id = id as usize;
        if let Some(generation) = self.generations.get(id) {
            Some(Entity::new(id as u32, *generation))
        } else {
            let free_cursor = self.free_cursor.load(Ordering::Relaxed);
            let num_pending = usize::try_from(-free_cursor).ok()?;
            (id < self.generations.len() - num_pending)
                .then_some(Entity::new(id as u32, NonZeroU32::MIN))
        }
    }
}
