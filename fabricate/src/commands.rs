use std::collections::VecDeque;

use anyhow::Result;

use crate::{registry::Entity, storage::Data, world::World};

pub enum Command {
    DespawnEntity(Entity),
    AddComponents(Entity, Vec<Data>),
}

#[derive(Default)]
pub struct Commands {
    queue: VecDeque<Command>,
    spawned_entities: Vec<Entity>,
}

impl Commands {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            spawned_entities: Vec::new(),
        }
    }

    pub fn create_entity(&mut self) {
        self.spawned_entities.push(Entity::allocate(None));
    }

    pub fn despawn(&mut self, entity: Entity) {
        self.enqueue(Command::DespawnEntity(entity));
    }

    pub fn add_components(&mut self, entity: &Entity, data: Vec<Data>) {
        self.enqueue(Command::AddComponents(entity.clone(), data));
    }

    pub fn enqueue(&mut self, command: Command) {
        self.queue.push_back(command);
    }

    pub fn dequeue(&mut self) -> Option<Command> {
        self.queue.pop_front()
    }

    pub(crate) fn finalize(&mut self, world: &mut World) -> Result<()> {
        for entity in self.spawned_entities.drain(..) {
            world.insert_entity(entity)?;
        }
        while let Some(command) = self.dequeue() {
            match command {
                Command::DespawnEntity(entity) => {
                    world.despawn(&entity);
                }
                Command::AddComponents(entity, data) => {
                    world.add_data(&entity, data)?;
                }
            }
        }

        Ok(())
    }
}
