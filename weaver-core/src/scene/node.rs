use crate::ecs::entity::Entity;
use petgraph::graph::NodeIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Node {
    pub(super) entity: Entity,
    pub(super) scene_index: NodeIndex,
}

impl Node {
    pub fn entity(&self) -> Entity {
        self.entity
    }

    pub fn scene_index(&self) -> NodeIndex {
        self.scene_index
    }
}
