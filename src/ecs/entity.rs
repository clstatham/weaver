/// A unique identifier for a collection of Components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Entity {
    id: u32,
}

impl Entity {
    /// A placeholder [Entity], used to indicate that a Component is not attached to any [Entity].
    pub const PLACEHOLDER: Entity = Entity { id: u32::MAX };

    /// Creates a new [Entity] with the given ID.
    pub fn new(id: u32) -> Entity {
        Entity { id }
    }
}
