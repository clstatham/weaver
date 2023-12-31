/// A unique identifier for a collection of components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Entity {
    id: u32,
    generation: u32,
}

impl Entity {
    pub const PLACEHOLDER: Self = Self {
        id: u32::MAX,
        generation: u32::MAX,
    };

    /// Creates a new entity with the given id and generation.
    pub fn new(id: u32, generation: u32) -> Self {
        Self { id, generation }
    }

    /// Returns the id of the entity.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Returns the generation of the entity.
    pub fn generation(&self) -> u32 {
        self.generation
    }
}
