use std::sync::Arc;

use crate::{
    bundle::Bundle,
    component::Component,
    entity::Entity,
    prelude::World,
    query::{Query, Queryable},
    system::{SystemAccess, SystemParam},
    world::ConstructFromWorld,
};

pub type CommandOp = dyn FnOnce(&mut World) -> Arc<dyn Component> + Send + Sync;

pub struct Command {
    pub(crate) op: Box<CommandOp>,
    pub(crate) tx: crossbeam_channel::Sender<Arc<dyn Component>>,
}

impl Command {
    pub fn run(self, world: &mut World) {
        let result = (self.op)(world);
        self.tx.try_send(result).unwrap();
    }
}

#[derive(Clone)]
pub struct Commands {
    pub(crate) tx: crossbeam_channel::Sender<Command>,
}

impl Commands {
    pub fn run<R: Component>(&self, op: impl FnOnce(&mut World) -> R + Send + Sync + 'static) -> R {
        let (tx, rx) = crossbeam_channel::bounded(1);

        self.tx
            .try_send(Command {
                op: Box::new(|world| {
                    let result = op(world);
                    Arc::new(result)
                }),
                tx,
            })
            .unwrap();

        let any: Arc<_> = rx.recv().unwrap();
        let arc: Arc<R> = any.downcast_arc().unwrap();
        Arc::try_unwrap(arc).unwrap_or_else(|_| unreachable!())
    }

    pub fn has_resource<T: Component>(&self) -> bool {
        self.run(move |world| world.has_resource::<T>())
    }

    pub fn insert_resource<T: Component>(&self, resource: T) {
        self.run(move |world| {
            world.insert_resource(resource);
        })
    }

    pub fn init_resource<T: Component + ConstructFromWorld>(&self) {
        self.run(move |world| {
            world.init_resource::<T>();
        })
    }

    pub fn remove_resource<T: Component>(&self) -> Option<T> {
        self.run(move |world| world.remove_resource::<T>())
    }

    pub fn insert_component<T: Component>(&self, entity: Entity, component: T) {
        self.run(move |world| {
            world.insert_component(entity, component);
        })
    }

    pub fn remove_component<T: Component>(&self, entity: Entity) -> Option<T> {
        self.run(move |world| world.remove_component::<T>(entity))
    }

    pub fn spawn<T: Bundle>(&self, bundle: T) -> Entity {
        self.run(move |world| world.spawn(bundle))
    }

    pub fn query<Q: Queryable + 'static>(&self) -> Query<Q> {
        self.run(move |world| world.query::<Q>())
    }
}

impl SystemParam for Commands {
    type Item = Commands;
    type State = ();

    fn access() -> SystemAccess {
        SystemAccess::default()
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _state: &Self::State) -> Self::Item {
        world.commands()
    }
}
