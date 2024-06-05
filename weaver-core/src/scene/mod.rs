pub mod node;
pub mod relationship;

use petgraph::prelude::*;

use crate::{
    ecs::{component::Component, entity::Entity, world::World},
    util::lock::SharedLock,
};

use self::{
    node::Node,
    relationship::{Relationship, RelationshipConnection},
};

pub struct Scene {
    world: SharedLock<World>,
    root_entity: Entity,
    root: NodeIndex,
    graph: StableDiGraph<Node, RelationshipConnection>,
}

impl Scene {
    pub fn new(world: SharedLock<World>) -> Self {
        let root_entity = world.write().create_entity();
        let mut graph: StableGraph<Node, RelationshipConnection> = StableDiGraph::new();
        let root = graph.add_node(Node {
            entity: root_entity,
            scene_index: NodeIndex::new(0),
        });
        graph[root].scene_index = root;
        Self {
            world,
            root_entity,
            root,
            graph,
        }
    }

    pub fn world(&self) -> &SharedLock<World> {
        &self.world
    }

    pub fn root_entity(&self) -> Entity {
        self.root_entity
    }

    pub fn root(&self) -> &Node {
        &self.graph[self.root]
    }

    pub fn graph(&self) -> &StableDiGraph<Node, RelationshipConnection> {
        &self.graph
    }

    pub fn graph_mut(&mut self) -> &mut StableDiGraph<Node, RelationshipConnection> {
        &mut self.graph
    }

    pub fn create_node(&mut self) -> Node {
        let entity = self.world.write().create_entity();
        self.add_node(entity)
    }

    pub fn create_node_with<T: Component>(&mut self, component: T) -> Node {
        let entity = self.world.write().create_entity();
        self.world.write().insert_component(entity, component);
        self.add_node(entity)
    }

    pub fn add_node(&mut self, entity: Entity) -> Node {
        let node = self.graph.add_node(Node {
            entity,
            scene_index: NodeIndex::new(0),
        });
        self.graph[node].scene_index = node;
        self.graph[node]
    }

    pub fn add_relationship<T: Relationship>(&mut self, from: Node, to: Node, weight: T) {
        let from = from.scene_index;
        let to = to.scene_index;
        let connection = RelationshipConnection::new(self.graph[from], self.graph[to], weight);
        self.graph.add_edge(from, to, connection);
    }

    pub fn remove_node(&mut self, node: Node) {
        self.graph.remove_node(node.scene_index);
    }

    pub fn remove_relationship(&mut self, from: Node, to: Node) -> Option<Box<dyn Relationship>> {
        let from = from.scene_index;
        let to = to.scene_index;
        if let Some(edge) = self.graph.find_edge(from, to) {
            let connection = self.graph.remove_edge(edge)?;
            Some(connection.weight)
        } else {
            None
        }
    }

    pub fn find_node(&self, entity: Entity) -> Option<Node> {
        self.graph
            .node_indices()
            .find(|&node| self.graph[node].entity == entity)
            .map(|node| self.graph[node])
    }

    pub fn children_of(&self, node: Node) -> impl Iterator<Item = Node> + '_ {
        self.graph
            .neighbors_directed(node.scene_index, Direction::Outgoing)
            .map(|node| self.graph[node])
    }

    pub fn parent_of(&self, node: Node) -> Option<Node> {
        self.graph
            .neighbors_directed(node.scene_index, Direction::Incoming)
            .map(|node| self.graph[node])
            .next()
    }

    pub fn siblings_of(&self, node: Node) -> Option<impl Iterator<Item = Node> + '_> {
        let parent = self.parent_of(node)?;
        Some(
            self.children_of(parent)
                .filter(move |sibling| *sibling != node),
        )
    }

    pub fn child_relationships_of(
        &self,
        node: Node,
    ) -> impl Iterator<Item = &dyn Relationship> + '_ {
        self.graph
            .edges_directed(node.scene_index, Direction::Outgoing)
            .map(move |edge| &*edge.weight().weight)
    }

    pub fn parent_relationship_of(&self, node: Node) -> Option<&dyn Relationship> {
        self.graph
            .edges_directed(node.scene_index, Direction::Incoming)
            .map(move |edge| &*edge.weight().weight)
            .next()
    }
}
