use std::{collections::VecDeque, fmt::Debug, sync::Arc};

use crate as weaver_ecs;
use crate::component::Data;
use crate::prelude::Component;

use crate::storage::Components;
use crate::{bundle::Bundle, entity::Entity, prelude::EntityGraph, world::World};
use parking_lot::RwLock;

pub enum Command {
    Despawn(Entity),
    DespawnRecursive(Entity),
    AddChild(Entity, Entity),
    RemoveChild(Entity, Entity),
    AddSibling(Entity, Entity),
    RemoveSibling(Entity, Entity),
}

#[derive(Component)]
pub struct Commands {
    components: Components,
    entity_graph: EntityGraph,
    tape: VecDeque<Command>,
}

impl Commands {
    pub fn new(world: Arc<RwLock<World>>) -> Self {
        let components = world.read().components.split();
        let entity_graph = world.read().read_resource::<EntityGraph>().unwrap().clone();
        Self {
            components,
            entity_graph,
            tape: VecDeque::new(),
        }
    }

    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> Entity {
        let entity = self.components.build(bundle);
        self.entity_graph.add_entity(entity);
        entity
    }

    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) {
        self.components.add_component(&entity, component, None);
    }

    pub fn add_dynamic_component(&mut self, entity: Entity, component: Data) {
        self.components.add_dynamic_component(&entity, component);
    }

    pub fn despawn(&mut self, entity: Entity) {
        if !self.components.despawn(entity.id()) {
            self.tape.push_back(Command::Despawn(entity));
        } else {
            self.entity_graph.remove_entity(entity);
        }
    }

    fn despawn_recursive_inner(&mut self, entity: Entity) -> bool {
        if !self.components.despawn(entity.id()) {
            self.tape.push_back(Command::DespawnRecursive(entity));
            return false;
        }
        for child in self.entity_graph.get_children(entity) {
            self.despawn_recursive_inner(child);
        }
        true
    }

    pub fn despawn_recursive(&mut self, entity: Entity) {
        if !self.despawn_recursive_inner(entity) {
            self.tape.push_back(Command::DespawnRecursive(entity));
        } else {
            self.entity_graph.remove_entity(entity);
        }
    }

    pub fn add_child(&mut self, parent: Entity, child: Entity) {
        self.tape.push_back(Command::AddChild(parent, child));
    }

    pub fn remove_child(&mut self, parent: Entity, child: Entity) {
        self.tape.push_back(Command::RemoveChild(parent, child));
    }

    pub fn add_sibling(&mut self, sibling: Entity, entity: Entity) {
        self.tape.push_back(Command::AddSibling(sibling, entity));
    }

    pub fn remove_sibling(&mut self, sibling: Entity, entity: Entity) {
        self.tape.push_back(Command::RemoveSibling(sibling, entity));
    }

    pub fn finalize(mut self, world: &mut World) {
        world.components.merge(self.components);
        for entity in self.entity_graph.graph.node_weights() {
            world
                .write_resource::<EntityGraph>()
                .unwrap()
                .add_entity(*entity);
        }

        while let Some(command) = self.tape.pop_front() {
            match command {
                Command::Despawn(entity) => {
                    world.despawn(entity);
                }
                Command::DespawnRecursive(entity) => {
                    world.despawn_recursive(entity);
                }
                Command::AddChild(parent, child) => {
                    world
                        .write_resource::<EntityGraph>()
                        .unwrap()
                        .add_child(parent, child);
                }
                Command::RemoveChild(parent, child) => {
                    world
                        .write_resource::<EntityGraph>()
                        .unwrap()
                        .remove_child(parent, child);
                }
                Command::AddSibling(sibling, entity) => {
                    world
                        .write_resource::<EntityGraph>()
                        .unwrap()
                        .add_sibling(sibling, entity);
                }
                Command::RemoveSibling(sibling, entity) => {
                    world
                        .write_resource::<EntityGraph>()
                        .unwrap()
                        .remove_sibling(sibling, entity);
                }
            }
        }
    }
}

impl Debug for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Commands").finish()
    }
}
