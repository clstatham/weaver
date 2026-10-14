//! Entity ids and the allocator that hands them out.

use std::{
    hash::{BuildHasherDefault, Hash, Hasher},
    num::NonZeroU32,
    sync::atomic::{AtomicI64, Ordering},
};

use weaver_util::prelude::*;

/// A generational entity id: a slot index plus a generation counter.
///
/// Destroying an entity bumps its slot's generation, so a stale `Entity` held somewhere else no
/// longer matches and [`Entities::is_alive`] reports false for it. Entities are created through
/// [`World::spawn`](crate::world::World::spawn) or
/// [`World::create_entity`](crate::world::World::create_entity), not by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct Entity {
    id: u32,
    generation: NonZeroU32,
}

impl Entity {
    /// Builds an entity from its parts. Doesn't allocate anything; use [`Entities`] for that.
    pub const fn new(id: u32, generation: NonZeroU32) -> Self {
        Self { id, generation }
    }

    /// The slot index. Unique among live entities, reused after the entity is destroyed.
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// How many times the slot has been freed, plus one. Starts at 1.
    pub const fn generation(&self) -> u32 {
        self.generation.get()
    }

    /// Packs the entity as `generation << 32 | id`.
    pub const fn as_u64(&self) -> u64 {
        ((self.generation() as u64) << 32) | self.id as u64
    }

    /// Unpacks an entity from the layout produced by [`Entity::as_u64`].
    ///
    /// # Panics
    ///
    /// Panics if the generation half is zero.
    pub fn from_u64(value: u64) -> Self {
        Self {
            id: value as u32,
            generation: NonZeroU32::new((value >> 32) as u32).unwrap(),
        }
    }

    /// [`Entity::as_u64`] truncated to `usize`.
    pub const fn as_usize(&self) -> usize {
        self.as_u64() as usize
    }

    /// Inverse of [`Entity::as_usize`].
    ///
    /// # Panics
    ///
    /// Panics if the generation half is zero.
    pub fn from_usize(value: usize) -> Self {
        Self::from_u64(value as u64)
    }
}

impl Hash for Entity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.as_u64());
    }
}

/// A hasher for [`Entity`] keys: one multiply on the packed id, no other mixing.
///
/// Only meant for [`EntityMap`] and [`EntitySet`]; it panics if fed arbitrary bytes.
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

/// A `HashMap` keyed by [`Entity`] using [`EntityHasher`].
pub type EntityMap<V> =
    std::collections::hash_map::HashMap<Entity, V, BuildHasherDefault<EntityHasher>>;

/// A `HashSet` of [`Entity`] using [`EntityHasher`].
pub type EntitySet = std::collections::hash_set::HashSet<Entity, BuildHasherDefault<EntityHasher>>;

/// The entity allocator: tracks which slots are alive and recycles freed ones.
///
/// Follows Bevy's scheme. Freed slots go on a `pending` list with their generation already bumped.
/// [`Entities::reserve`] works through `&self` (it moves an atomic cursor), so ids can be handed
/// out without exclusive access; the reservation is made real by the next [`Entities::flush`],
/// which every `&mut self` method does first.
#[derive(Debug, Default)]
pub struct Entities {
    // Weaver uses a system similar to Bevy's entity system to manage entities.
    free_cursor: AtomicI64,
    pending: Vec<u32>,
    generations: Vec<NonZeroU32>,
}

impl Entities {
    /// Whether there are reservations that haven't been flushed yet.
    pub fn needs_flush(&mut self) -> bool {
        *self.free_cursor.get_mut() != self.pending.len() as i64
    }

    // pub fn verify_flushed(&mut self) {
    //     assert!(
    //         !self.needs_flush(),
    //         "Entities::flush() must be called before this operation"
    //     );
    // }

    /// Makes outstanding [`Entities::reserve`] calls real: grows the generation table for fresh
    /// ids and drops the recycled ids from the pending list.
    pub fn flush(&mut self) {
        let free_cursor = self.free_cursor.get_mut();
        let current_free_cursor = *free_cursor;
        if *free_cursor == self.pending.len() as i64 {
            return; // flush not needed
        }

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

    /// Hands out an entity id through `&self`, preferring a recycled slot.
    ///
    /// The entity counts as alive immediately but isn't fully allocated until
    /// [`Entities::flush`] runs.
    ///
    /// # Panics
    ///
    /// Panics if the id space (`u32`) is exhausted.
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

    /// Allocates an entity, reusing a freed slot if there is one.
    ///
    /// # Panics
    ///
    /// Panics if the id space (`u32`) is exhausted.
    pub fn alloc(&mut self) -> Entity {
        self.flush();

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

    /// Marks exactly `entity` (id and generation) as alive, growing the table if needed and
    /// pulling its slot off the free list. Used to recreate entities with known ids.
    pub fn alloc_at(&mut self, entity: Entity) {
        self.flush();

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

    /// Frees the entity's slot and bumps its generation, so the old `Entity` stops being alive.
    /// Does nothing if the entity is already dead.
    ///
    /// # Panics
    ///
    /// Panics if the entity's id was never allocated.
    pub fn free(&mut self, entity: Entity) {
        self.flush();

        let generation = &mut self.generations[entity.id as usize];
        if generation.get() != entity.generation() {
            // entity is already dead
            return;
        }

        // FIXME: panics when the generation is u32::MAX (wraps to 0, not NonZero); the wrap-around
        // warning below never fires
        *generation = NonZeroU32::new(generation.get().wrapping_add(1)).unwrap();

        if *generation == NonZeroU32::MIN {
            log::warn!("Entity generation wrapped around on entity {:?}", entity.id);
        }

        self.pending.push(entity.id);

        let new_free_cursor = self.pending.len() as i64;
        *self.free_cursor.get_mut() = new_free_cursor;
    }

    /// Returns true if the entity is allocated (or reserved) with this generation.
    pub fn is_alive(&self, entity: Entity) -> bool {
        // `free` bumps the generation right away, so a dead entity never matches
        match self.generations.get(entity.id as usize) {
            Some(generation) => *generation == entity.generation,
            None => self.find_by_id(entity.id) == Some(entity),
        }
    }

    /// The entity currently occupying slot `id`, with the slot's current generation, if the slot
    /// exists (allocated or reserved). Doesn't check that the slot hasn't been freed.
    pub fn find_by_id(&self, id: u32) -> Option<Entity> {
        let id = id as usize;
        if let Some(generation) = self.generations.get(id) {
            Some(Entity::new(id as u32, *generation))
        } else {
            let free_cursor = self.free_cursor.load(Ordering::Relaxed);
            let num_pending = usize::try_from(-free_cursor).ok()?;
            // reserved but unflushed ids sit just past the end of `generations`
            (id < self.generations.len() + num_pending)
                .then_some(Entity::new(id as u32, NonZeroU32::MIN))
        }
    }
}
