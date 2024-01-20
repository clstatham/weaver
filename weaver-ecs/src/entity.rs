pub type EntityId = u32;

/// A unique identifier for a collection of components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Entity {
    id: EntityId,
}

impl Entity {
    pub const PLACEHOLDER: Self = Self { id: EntityId::MAX };

    /// Creates a new entity with the given id and generation.
    pub fn new(id: EntityId) -> Self {
        Self { id }
    }

    /// Returns the id of the entity.
    pub fn id(&self) -> EntityId {
        self.id
    }
}
