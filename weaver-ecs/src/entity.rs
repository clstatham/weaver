use crate::id::DynamicId;

/// A unique identifier for a collection of components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Entity {
    id: DynamicId,
}

impl Entity {
    pub const PLACEHOLDER: Self = Self { id: DynamicId::MAX };

    /// Creates a new entity with the given id.
    pub fn new(id: DynamicId) -> Self {
        Self { id }
    }

    /// Returns the id of the entity.
    pub fn id(&self) -> DynamicId {
        self.id
    }
}
