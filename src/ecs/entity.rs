/// A unique identifier for a collection of Components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Entity {
    id: u32,
    generation: u32,
}

impl Entity {
    /// A placeholder [Entity], used to indicate that a Component is not attached to any [Entity].
    pub const PLACEHOLDER: Entity = Entity {
        id: u32::MAX,
        generation: u32::MAX,
    };

    /// Creates a new [Entity] with the given ID and generation 0.
    pub fn new(id: u32) -> Entity {
        Entity { id, generation: 0 }
    }

    /// Returns the ID of this [Entity].
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Returns the generation of this [Entity].
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Returns `true` if this [Entity] is a placeholder.
    pub fn is_placeholder(&self) -> bool {
        self.id == u32::MAX
    }

    /// Returns a new [Entity] with the same ID and the next generation.
    pub fn next_generation(&self) -> Entity {
        Entity {
            id: self.id,
            generation: self.generation + 1,
        }
    }
}
