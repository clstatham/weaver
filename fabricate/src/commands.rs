use std::{collections::VecDeque, sync::Arc};

use anyhow::Result;

use crate::{
    bundle::Bundle,
    lock::SharedLock,
    registry::Entity,
    relationship::Relationship,
    storage::Data,
    system::{System, SystemStage},
    world::{LockedWorldHandle, World},
};

pub enum Command {
    SpawnEntity(Entity),
    DespawnEntity(Entity),
    AddComponents(Entity, Vec<Data>),
    AddSystem(SystemStage, Arc<dyn System>),
    GarbageCollect,
}

/// Commands to apply to a mutable world after a [`System`][crate::system::System] or [`defer`][crate::world::LockedWorldHandle::defer] block.
#[derive(Clone)]
pub struct Commands {
    pub(crate) world: LockedWorldHandle,
    pub(crate) queue: SharedLock<VecDeque<Command>>,
}

impl Commands {
    /// Create a new, empty command queue.
    pub fn new(world: LockedWorldHandle) -> Self {
        Self {
            world,
            queue: SharedLock::new(VecDeque::new()),
        }
    }

    pub fn world(&self) -> &LockedWorldHandle {
        &self.world
    }

    /// Enqueues a command to spawn an entity.
    ///
    /// # Note
    ///
    /// The entity will not be visible on the [`World`] until [`finalize`][Commands::finalize]is called.
    pub fn create_entity(&mut self) -> Entity {
        let e = Entity::allocate(None);
        self.queue.write().push_back(Command::SpawnEntity(e));
        e
    }

    pub fn spawn<B: Bundle>(&mut self, bundle: B) -> Entity {
        let entity = self.create_entity();
        self.add(entity, bundle);
        entity
    }

    /// Enqueues a command to despawn an entity.
    ///
    /// # Note
    ///
    /// The entity will not be removed from the [`World`] until [`finalize`][Commands::finalize]is called.
    pub fn despawn(&mut self, entity: Entity) {
        self.queue.write().push_back(Command::DespawnEntity(entity));
    }

    /// Enqueues a command to add components to an entity.
    ///
    /// # Note
    ///
    /// The components will not be visible on the [`World`] until [`finalize`][Commands::finalize]is called.
    pub fn add<B: Bundle>(&mut self, entity: Entity, components: B) {
        self.queue.write().push_back(Command::AddComponents(
            entity,
            components.into_data_vec(&self.world),
        ));
    }

    pub fn add_relationship<R: Relationship>(
        &mut self,
        entity: Entity,
        relationship: R,
        relative: Entity,
    ) {
        self.queue.write().push_back(Command::AddComponents(
            entity,
            vec![relationship.into_relationship_data(&self.world, relative)],
        ));
    }

    /// Enqueues a command to add a system to a stage.
    ///
    /// # Note
    ///
    /// The system will not be visible on the [`World`] until [`finalize`][Commands::finalize]is called.
    pub fn add_system(&mut self, stage: SystemStage, system: impl System) {
        self.queue
            .write()
            .push_back(Command::AddSystem(stage, Arc::new(system)));
    }

    /// Enqueues a command to garbage collect the world.
    ///
    /// This will remove any entities that have been despawned.
    ///
    /// # Note
    ///
    /// The garbage collection will not be visible on the [`World`] until [`finalize`][Commands::finalize]is called.
    pub fn garbage_collect(&mut self) {
        self.queue.write().push_back(Command::GarbageCollect);
    }

    /// Finalizes the command queue, applying all enqueued commands to the world.
    pub(crate) fn finalize(&mut self, world: &mut World) -> Result<()> {
        while let Some(command) = self.queue.write().pop_front() {
            match command {
                Command::SpawnEntity(e) => {
                    world.insert_entity(e)?;
                }
                Command::DespawnEntity(entity) => {
                    world.despawn(entity);
                }
                Command::AddComponents(entity, data) => {
                    world.add_data(entity, data)?;
                }
                Command::AddSystem(stage, system) => {
                    world.add_system(stage, system);
                }
                Command::GarbageCollect => {
                    world.garbage_collect();
                }
            }
        }

        Ok(())
    }
}
