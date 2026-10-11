use weaver_util::lock::SharedLock;

use crate::{
    bundle::Bundle,
    change_detection::TickRange,
    component::Component,
    entity::{Entities, Entity},
    prelude::World,
    system::{SystemAccess, SystemParam},
    world::ConstructFromWorld,
};

pub type Command = dyn FnOnce(&mut World) + Send + 'static;
pub type BoxedCommand = Box<Command>;

pub struct CommandQueue {
    tx: crossbeam_channel::Sender<BoxedCommand>,
    rx: crossbeam_channel::Receiver<BoxedCommand>,
    entities: SharedLock<Entities>,
}

impl CommandQueue {
    pub fn new(world: &World) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            tx,
            rx,
            entities: world.entities_handle(),
        }
    }
}

#[derive(Clone)]
pub struct Commands {
    pub(crate) tx: crossbeam_channel::Sender<BoxedCommand>,
    entities: SharedLock<Entities>,
}

impl Commands {
    pub fn add(&self, op: impl FnOnce(&mut World) + Send + 'static) {
        self.tx.send(Box::new(op)).unwrap();
    }

    pub fn insert_resource<T: Component>(&self, resource: T) {
        self.add(move |world| {
            world.insert_resource(resource);
        })
    }

    pub fn remove_resource<T: Component>(&self) {
        self.add(move |world| {
            world.remove_resource::<T>();
        });
    }

    pub fn init_resource<T: Component + ConstructFromWorld>(&self) {
        self.add(move |world| {
            world.init_resource::<T>();
        })
    }

    pub fn insert_component<T: Component>(&self, entity: Entity, component: T) {
        self.add(move |world| {
            world.insert_component(entity, component);
        })
    }

    /// Inserts a bundle of components into the entity, replacing any it already has.
    pub fn insert_bundle<T: Bundle>(&self, entity: Entity, bundle: T) {
        self.add(move |world| {
            world.insert_bundle(entity, bundle);
        })
    }

    /// Destroys the entity and all of its components. Does nothing if it's already dead.
    pub fn despawn(&self, entity: Entity) {
        self.add(move |world| {
            world.destroy_entity(entity);
        })
    }

    pub fn remove_component<T: Component>(&self, entity: Entity) {
        self.add(move |world| {
            world.remove_component::<T>(entity);
        });
    }

    pub fn spawn<T: Bundle>(&self, bundle: T) -> Entity {
        let entity = self.entities.read().reserve();
        self.add(move |world| world.insert_bundle(entity, bundle));
        entity
    }
}

impl SystemParam for Commands {
    type Item = Commands;
    type State = CommandQueue;

    fn access() -> SystemAccess {
        SystemAccess::default()
    }

    fn init_state(world: &World) -> Self::State {
        CommandQueue::new(world)
    }

    fn fetch(_world: &World, state: &Self::State, _ticks: TickRange) -> Self::Item {
        Commands {
            tx: state.tx.clone(),
            entities: state.entities.clone(),
        }
    }

    fn apply(state: &mut Self::State, world: &mut World) {
        world.flush_entities();
        for command in state.rx.try_iter() {
            command(world);
        }
    }
}
