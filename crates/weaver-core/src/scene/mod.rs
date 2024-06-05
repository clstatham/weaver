pub mod node;
pub mod relationship;

use std::rc::Rc;

use petgraph::prelude::*;

use crate::{
    ecs::{component::Component, entity::Entity, world::World},
    util::lock::Lock,
};

use self::{
    node::Node,
    relationship::{Relationship, RelationshipConnection},
};

pub struct Scene {
    world: Rc<World>,
    root_entity: Entity,
    root: NodeIndex,
    graph: Lock<StableDiGraph<Node, RelationshipConnection>>,
}

impl Scene {
    pub fn new(world: Rc<World>) -> Self {
        let root_entity = world.create_entity();
        let mut graph = StableDiGraph::new();
        let root = graph.add_node(Node {
            entity: root_entity,
            scene_index: NodeIndex::new(0),
        });
        graph[root].scene_index = root;
        Self {
            world,
            root_entity,
            root,
            graph: Lock::new(graph),
        }
    }

    pub fn world(&self) -> &Rc<World> {
        &self.world
    }

    pub fn root_entity(&self) -> Entity {
        self.root_entity
    }

    pub fn root(&self) -> Node {
        self.graph.read()[self.root]
    }

    pub fn graph(&self) -> &Lock<StableDiGraph<Node, RelationshipConnection>> {
        &self.graph
    }

    pub fn create_node(&self) -> Node {
        let entity = self.world.create_entity();
        self.add_node(entity)
    }

    pub fn create_node_with<T: Component>(&self, component: T) -> Node {
        let entity = self.world.create_entity();
        self.world.insert_component(entity, component);
        self.add_node(entity)
    }

    pub fn add_node(&self, entity: Entity) -> Node {
        let node_scene_index = self.graph.write().add_node(Node {
            entity,
            scene_index: NodeIndex::new(0),
        });
        self.graph.write()[node_scene_index].scene_index = node_scene_index;
        self.graph.read()[node_scene_index]
    }

    pub fn add_relationship<T: Relationship>(&self, from: Node, to: Node, weight: T) {
        let from = from.scene_index;
        let to = to.scene_index;
        let connection =
            RelationshipConnection::new(self.graph.read()[from], self.graph.read()[to], weight);
        self.graph.write().add_edge(from, to, connection);
    }

    pub fn create_sub_scene(&self) -> Scene {
        let sub_scene = Scene::new(self.world.clone());
        self.add_node(sub_scene.root_entity);
        sub_scene
    }

    pub fn remove_node(&self, node: Node) {
        self.graph.write().remove_node(node.scene_index);
    }

    pub fn remove_relationship(&mut self, from: Node, to: Node) -> Option<Box<dyn Relationship>> {
        let from = from.scene_index;
        let to = to.scene_index;
        if let Some(edge) = self.graph.read().find_edge(from, to) {
            let connection = self.graph.write().remove_edge(edge)?;
            Some(connection.weight)
        } else {
            None
        }
    }

    pub fn find_node(&self, entity: Entity) -> Option<Node> {
        self.graph
            .read()
            .node_indices()
            .find(|&node| self.graph.read()[node].entity == entity)
            .map(|node| self.graph.read()[node])
    }

    pub fn children_of(&self, node: Node) -> Vec<Node> {
        self.graph
            .read()
            .neighbors_directed(node.scene_index, Direction::Outgoing)
            .map(|node| self.graph.read()[node])
            .collect()
    }

    pub fn parent_of(&self, node: Node) -> Option<Node> {
        self.graph
            .read()
            .neighbors_directed(node.scene_index, Direction::Incoming)
            .map(|node| self.graph.read()[node])
            .next()
    }

    pub fn siblings_of(&self, node: Node) -> Option<Vec<Node>> {
        let parent = self.parent_of(node)?;
        Some(
            self.children_of(parent)
                .into_iter()
                .filter(move |sibling| *sibling != node)
                .collect(),
        )
    }
}
