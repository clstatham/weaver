//! Deferred world mutation. [`Commands`] queues closures that run later with `&mut World`.

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

/// A queued world mutation.
pub type Command = dyn FnOnce(&mut World) + Send + 'static;
/// A boxed [`Command`].
pub type BoxedCommand = Box<Command>;

/// The state behind a system's [`Commands`] parameter: the channel its commands go through.
///
/// [`Commands::apply`](SystemParam::apply) drains it with `&mut World` once the system's layer is
/// done.
pub struct CommandQueue {
    tx: crossbeam_channel::Sender<BoxedCommand>,
    rx: crossbeam_channel::Receiver<BoxedCommand>,
    entities: SharedLock<Entities>,
}

impl CommandQueue {
    /// Creates an empty queue that reserves entities from `world`.
    pub fn new(world: &World) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            tx,
            rx,
            entities: world.entities_handle(),
        }
    }
}

/// Queues changes to the world to be applied after the current layer of systems.
///
/// A system can't add or remove entities or components while its queries are alive, and other
/// systems in the layer may be holding borrows too, so structural changes are recorded here
/// instead. They run in the order they were issued, with `&mut World`, after every system in the
/// layer has finished. Systems in the next layer see the result.
///
/// [`spawn`](Commands::spawn) is the one call with an immediate result: it reserves the
/// [`Entity`] right away so it can be used in later commands, but the components aren't attached
/// until the queue is applied.
///
/// # Examples
///
/// ```
/// use weaver_ecs::prelude::*;
///
/// struct Health(u32);
///
/// fn spawn_enemy(commands: Commands) {
///     let enemy = commands.spawn((Health(10),));
///     commands.insert_component(enemy, 3u8);
/// }
/// # let _ = spawn_enemy;
/// ```
#[derive(Clone)]
pub struct Commands {
    pub(crate) tx: crossbeam_channel::Sender<BoxedCommand>,
    entities: SharedLock<Entities>,
}

impl Commands {
    /// Queues an arbitrary closure to run with `&mut World` when the queue is applied.
    ///
    /// # Panics
    ///
    /// Panics if the queue has already been dropped, which happens when the system that owned
    /// these `Commands` has been removed.
    pub fn add(&self, op: impl FnOnce(&mut World) + Send + 'static) {
        self.tx.send(Box::new(op)).unwrap();
    }

    /// Queues [`World::insert_resource`].
    pub fn insert_resource<T: Component>(&self, resource: T) {
        self.add(move |world| {
            world.insert_resource(resource);
        })
    }

    /// Queues [`World::remove_resource`].
    pub fn remove_resource<T: Component>(&self) {
        self.add(move |world| {
            world.remove_resource::<T>();
        });
    }

    /// Queues [`World::init_resource`], which builds the resource from the world if it is missing.
    pub fn init_resource<T: Component + ConstructFromWorld>(&self) {
        self.add(move |world| {
            world.init_resource::<T>();
        })
    }

    /// Queues inserting `component` on `entity`, replacing a component of the same type.
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

    /// Queues removing the `T` component from `entity`.
    pub fn remove_component<T: Component>(&self, entity: Entity) {
        self.add(move |world| {
            world.remove_component::<T>(entity);
        });
    }

    /// Spawns an entity with the components in `bundle` and returns its id.
    ///
    /// The id is reserved immediately and can be passed to other commands, but the entity doesn't
    /// get its components until the queue is applied.
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
