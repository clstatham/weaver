pub type EntityId = u32;

/// A unique identifier for a collection of components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct Entity {
    id: EntityId,
    generation: u32,
}

impl Entity {
    pub const PLACEHOLDER: Self = Self {
        id: EntityId::MAX,
        generation: u32::MAX,
    };

    /// Creates a new entity with the given id and generation.
    pub fn new(id: EntityId, generation: u32) -> Self {
        Self { id, generation }
    }

    /// Returns the id of the entity.
    pub fn id(&self) -> EntityId {
        self.id
    }

    /// Returns the generation of the entity.
    pub fn generation(&self) -> u32 {
        self.generation
    }
}
