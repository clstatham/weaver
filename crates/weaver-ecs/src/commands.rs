use std::ops::{Deref, DerefMut};

use crate::prelude::{
    Bundle, Component, Entity, FromWorld, Resource, SystemParam, UnsafeWorldCell,
};

use weaver_util::lock::SharedLock;

use crate::prelude::World;

pub trait Command: 'static + Send + Sync {
    fn execute(self: Box<Self>, world: &mut World);
}

impl<F> Command for F
where
    F: FnOnce(&mut World) + Send + Sync + 'static,
{
    fn execute(self: Box<Self>, world: &mut World) {
        self(world)
    }
}

#[derive(Default)]
pub struct CommandQueue {
    commands: Vec<Box<dyn Command>>,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push<T: Command>(&mut self, command: T) {
        self.commands.push(Box::new(command));
    }

    pub fn execute(&mut self, world: &mut World) {
        for command in self.commands.drain(..) {
            command.execute(world);
        }
    }
}

pub struct Commands {
    queue: SharedLock<CommandQueue>,
}

impl Commands {
    pub fn push<T: Command>(&self, command: T) {
        self.queue.write().push(command);
    }

    pub fn execute(&self, world: &mut World) {
        self.queue.write().execute(world);
    }

    pub fn insert_component(&self, entity: Entity, component: impl Component) {
        self.push(move |world: &mut World| {
            world.insert_component(entity, component);
        });
    }

    pub fn insert_components(&self, entity: Entity, bundle: impl Bundle) {
        self.push(move |world: &mut World| {
            world.insert_components(entity, bundle);
        });
    }

    pub fn remove_component<T: Component>(&self, entity: Entity) {
        self.push(move |world: &mut World| {
            world.remove_component::<T>(entity);
        });
    }

    pub fn init_resource<T: Resource + FromWorld>(&self) {
        self.push(move |world: &mut World| {
            world.init_resource::<T>();
        });
    }

    pub fn insert_resource<T: Resource>(&self, resource: T) {
        self.push(move |world: &mut World| {
            world.insert_resource(resource);
        });
    }

    pub fn remove_resource<T: Resource>(&self) {
        self.push(move |world: &mut World| {
            world.remove_resource::<T>();
        });
    }

    pub fn spawn<T: Bundle>(&self, bundle: T) {
        self.push(move |world: &mut World| {
            world.spawn(bundle);
        });
    }
}

unsafe impl SystemParam for Commands {
    type State = SharedLock<CommandQueue>;
    type Item<'w, 's> = Commands;

    fn validate_access(access: &crate::prelude::SystemAccess) -> bool {
        !access.exclusive
    }

    fn init_state(_world: &mut World) -> Self::State {
        SharedLock::new(CommandQueue::new())
    }

    fn access() -> crate::prelude::SystemAccess {
        crate::prelude::SystemAccess {
            exclusive: false,
            ..Default::default()
        }
    }

    unsafe fn fetch<'w, 's>(
        state: &'s mut Self::State,
        _world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        Commands {
            queue: state.clone(),
        }
    }

    #[inline(never)]
    fn apply(state: &mut Self::State, world: &mut World) {
        let commands = Commands {
            queue: state.clone(),
        };
        commands.execute(world);
    }
}

pub struct WorldMut<'w> {
    world: &'w mut World,
}

impl<'w> WorldMut<'w> {
    pub fn into_inner(self) -> &'w mut World {
        self.world
    }
}

unsafe impl SystemParam for WorldMut<'_> {
    type State = ();
    type Item<'w, 's> = WorldMut<'w>;

    fn validate_access(_access: &crate::prelude::SystemAccess) -> bool {
        true
    }

    fn init_state(_world: &mut World) -> Self::State {}

    fn access() -> crate::prelude::SystemAccess {
        crate::prelude::SystemAccess {
            exclusive: true,
            ..Default::default()
        }
    }

    unsafe fn fetch<'w, 's>(
        _state: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        WorldMut {
            world: unsafe { world.world_mut() },
        }
    }

    fn can_run(_world: &World) -> bool {
        true
    }
}

impl Deref for WorldMut<'_> {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        self.world
    }
}

impl DerefMut for WorldMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.world
    }
}
