use crate::ecs::entity::Entity;
use petgraph::graph::NodeIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Node {
    pub entity: Entity,
    pub scene_index: NodeIndex,
}
