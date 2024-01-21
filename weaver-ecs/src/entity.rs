use crate::{id::DynamicId, prelude::Component};

/// A unique identifier for a collection of components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Entity {
    id: DynamicId,
    generation: u32,
}

impl Entity {
    pub const PLACEHOLDER: Self = Self {
        id: DynamicId::MAX,
        generation: u32::MAX,
    };

    /// Creates a new entity with the given id.
    pub fn new(id: DynamicId) -> Self {
        Self { id, generation: 0 }
    }

    pub(crate) fn new_with_generation(id: DynamicId, generation: u32) -> Self {
        Self { id, generation }
    }

    /// Returns the id of the entity.
    pub fn id(&self) -> DynamicId {
        self.id
    }

    /// Returns the generation of the entity.
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Returns the entity as a u64. The upper 32 bits are the generation, and the lower 32 bits are the id.
    pub const fn as_u64(&self) -> u64 {
        ((self.generation as u64) << 32) | (self.id as u64)
    }

    /// Creates an entity from a u64. The upper 32 bits are the generation, and the lower 32 bits are the id.
    pub const fn from_u64(id: u64) -> Self {
        Self {
            id: (id & 0xFFFF_FFFF) as u32,
            generation: (id >> 32) as u32,
        }
    }
}

impl Component for Entity {}
