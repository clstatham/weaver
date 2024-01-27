use petgraph::prelude::*;
use rustc_hash::FxHashMap;

use crate::{prelude::Component, registry::DynamicId};

/// A unique identifier for a collection of components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Entity {
    id: DynamicId,
    generation: u32,
}

impl Entity {
    pub const PLACEHOLDER: Self = Self {
        id: DynamicId::MAX,
        generation: u32::MAX,
    };

    /// Creates a new entity with the given id. The generation is set to 0.
    pub fn new(id: DynamicId) -> Self {
        Self { id, generation: 0 }
    }

    /// Creates a new entity with the given id and generation.
    pub(crate) fn new_with_generation(id: DynamicId, generation: u32) -> Self {
        Self { id, generation }
    }

    /// Returns the id of the entity.
    pub fn id(&self) -> DynamicId {
        self.id
    }

    /// Returns the generation of the entity.
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Returns the entity as a u64. The upper 32 bits are the generation, and the lower 32 bits are the id.
    pub const fn as_u64(&self) -> u64 {
        ((self.generation as u64) << 32) | (self.id as u64)
    }

    /// Creates an entity from a u64. The upper 32 bits are the generation, and the lower 32 bits are the id.
    pub const fn from_u64(id: u64) -> Self {
        Self {
            id: (id & 0xFFFF_FFFF) as u32,
            generation: (id >> 32) as u32,
        }
    }
}

impl Component for Entity {
    fn type_name() -> &'static str {
        "Entity"
    }
}

#[derive(Default)]
pub struct EntityGraph {
    pub graph: StableDiGraph<Entity, ()>,
    indices_to_entities: FxHashMap<NodeIndex, Entity>,
    entities_to_indices: FxHashMap<Entity, NodeIndex>,
}

impl EntityGraph {
    pub fn roots(&self) -> Vec<Entity> {
        self.graph
            .node_indices()
            .filter(|id| {
                self.graph
                    .neighbors_directed(*id, petgraph::Direction::Incoming)
                    .count()
                    == 0
            })
            .map(|id| self.indices_to_entities.get(&id).copied().unwrap())
            .collect::<Vec<_>>()
    }

    #[allow(clippy::map_entry)]
    pub fn add_entity(&mut self, entity: Entity) {
        if !self.entities_to_indices.contains_key(&entity) {
            let id = self.graph.add_node(entity);
            self.entities_to_indices.insert(entity, id);
            self.indices_to_entities.insert(id, entity);
        }
    }

    pub fn add_relation(&mut self, parent: Entity, child: Entity) -> bool {
        if let Some(parent_id) = self.entities_to_indices.get(&parent).copied() {
            if let Some(child_id) = self.entities_to_indices.get(&child).copied() {
                if self.graph.contains_edge(parent_id, child_id) {
                    false
                } else {
                    self.graph.add_edge(parent_id, child_id, ());
                    true
                }
            } else {
                let child_id = self.graph.add_node(child);
                self.entities_to_indices.insert(child, child_id);
                self.indices_to_entities.insert(child_id, child);
                if self.graph.contains_edge(parent_id, child_id) {
                    false
                } else {
                    self.graph.add_edge(parent_id, child_id, ());
                    true
                }
            }
        } else {
            let parent_id = self.graph.add_node(parent);
            self.entities_to_indices.insert(parent, parent_id);
            self.indices_to_entities.insert(parent_id, parent);

            if let Some(child_id) = self.entities_to_indices.get(&child) {
                if self.graph.contains_edge(parent_id, *child_id) {
                    false
                } else {
                    self.graph.add_edge(parent_id, *child_id, ());
                    true
                }
            } else {
                let child_id = self.graph.add_node(child);
                self.entities_to_indices.insert(child, child_id);
                self.indices_to_entities.insert(child_id, child);
                if self.graph.contains_edge(parent_id, child_id) {
                    false
                } else {
                    self.graph.add_edge(parent_id, child_id, ());
                    true
                }
            }
        }
    }

    pub fn remove_relation(&mut self, parent: Entity, child: Entity) {
        if let Some(parent_id) = self.entities_to_indices.get(&parent) {
            if let Some(child_id) = self.entities_to_indices.get(&child) {
                let edges = self
                    .graph
                    .edges_connecting(*parent_id, *child_id)
                    .map(|edge| edge.id())
                    .collect::<Vec<_>>();
                for edge in edges {
                    self.graph.remove_edge(edge);
                }
            }
        }
    }

    pub fn get_children(&self, parent: Entity) -> Vec<Entity> {
        if let Some(parent_id) = self.entities_to_indices.get(&parent) {
            self.graph
                .neighbors_directed(*parent_id, petgraph::Direction::Outgoing)
                .map(|id| self.indices_to_entities.get(&id).copied().unwrap())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    }

    pub fn get_parent(&self, child: Entity) -> Option<Entity> {
        if let Some(child_id) = self.entities_to_indices.get(&child) {
            self.graph
                .neighbors_directed(*child_id, petgraph::Direction::Incoming)
                .map(|id| self.indices_to_entities.get(&id).copied().unwrap())
                .next()
        } else {
            None
        }
    }

    pub fn get_all_children(&self, parent: Entity) -> Vec<Entity> {
        if let Some(parent_id) = self.entities_to_indices.get(&parent) {
            let mut children = Vec::new();
            let mut stack = vec![*parent_id];
            while let Some(id) = stack.pop() {
                for child_id in self
                    .graph
                    .neighbors_directed(id, petgraph::Direction::Outgoing)
                {
                    stack.push(child_id);
                    children.push(self.indices_to_entities.get(&child_id).copied().unwrap());
                }
            }
            children
        } else {
            Vec::new()
        }
    }

    pub fn remove_entity(&mut self, entity: Entity) {
        if let Some(entity_id) = self.entities_to_indices.remove(&entity) {
            let edges = self
                .graph
                .edges(entity_id)
                .map(|edge| edge.id())
                .collect::<Vec<_>>();
            for edge in edges {
                self.graph.remove_edge(edge);
            }
            self.graph.remove_node(entity_id);
            self.indices_to_entities.remove(&entity_id);
        }
    }

    pub fn get_all_parents(&self, child: Entity) -> Vec<Entity> {
        if let Some(child_id) = self.entities_to_indices.get(&child) {
            let mut parents = Vec::new();
            let mut stack = vec![*child_id];
            while let Some(id) = stack.pop() {
                for parent_id in self
                    .graph
                    .neighbors_directed(id, petgraph::Direction::Incoming)
                {
                    stack.push(parent_id);
                    parents.push(self.indices_to_entities.get(&parent_id).copied().unwrap());
                }
            }
            parents
        } else {
            Vec::new()
        }
    }

    pub fn get_all_relations(&self, entity: Entity) -> Vec<Entity> {
        if let Some(entity_id) = self.entities_to_indices.get(&entity) {
            let mut relations = Vec::new();
            let mut stack = vec![*entity_id];
            while let Some(id) = stack.pop() {
                for child_id in self
                    .graph
                    .neighbors_directed(id, petgraph::Direction::Outgoing)
                {
                    stack.push(child_id);
                    relations.push(self.indices_to_entities.get(&child_id).copied().unwrap());
                }
                for parent_id in self
                    .graph
                    .neighbors_directed(id, petgraph::Direction::Incoming)
                {
                    stack.push(parent_id);
                    relations.push(self.indices_to_entities.get(&parent_id).copied().unwrap());
                }
            }
            relations
        } else {
            Vec::new()
        }
    }
}

impl Component for EntityGraph {
    fn type_name() -> &'static str {
        "EntityGraph"
    }
}
