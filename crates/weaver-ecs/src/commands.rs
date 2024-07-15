use crate::prelude::{Bundle, Component, Entity, Res, Resource, SystemParam, UnsafeWorldCell};

use weaver_util::SyncCell;

use crate::prelude::World;

pub trait Command: Send + Sync + 'static {
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

pub struct Commands<'w, 's> {
    queue: &'s mut CommandQueue,
    world: &'w World,
}

impl<'w, 's> Commands<'w, 's> {
    pub fn push<T: Command>(&mut self, command: T) {
        self.queue.push(command);
    }

    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> Entity {
        let entity = self.world.entities().reserve();
        self.push(move |world: &mut World| {
            world.insert_bundle(entity, bundle);
        });
        entity
    }

    pub fn insert_component<T: Component>(&mut self, entity: Entity, component: T) {
        self.push(move |world: &mut World| {
            world.insert_component(entity, component);
        });
    }

    pub fn insert_bundle<B: Bundle>(&mut self, entity: Entity, bundle: B) {
        self.push(move |world: &mut World| {
            world.insert_bundle(entity, bundle);
        });
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        self.push(move |world: &mut World| {
            world.remove_component::<T>(entity);
        });
    }

    pub fn insert_resource<T: Resource + Send + Sync>(&mut self, resource: T) {
        self.push(move |world: &mut World| {
            world.insert_resource(resource);
        });
    }

    pub fn remove_resource<T: Resource>(&mut self) {
        self.push(move |world: &mut World| {
            world.remove_resource::<T>();
        });
    }

    pub fn get_resource<T: Resource>(&self) -> Option<Res<T>> {
        self.world.get_resource::<T>()
    }
}

unsafe impl SystemParam for Commands<'_, '_> {
    type State = SyncCell<CommandQueue>;
    type Item<'w, 's> = Commands<'w, 's>;

    fn validate_access(access: &crate::prelude::SystemAccess) -> bool {
        !access.exclusive
    }

    fn init_state(_world: &mut World) -> Self::State {
        SyncCell::new(CommandQueue::new())
    }

    fn access() -> crate::prelude::SystemAccess {
        crate::prelude::SystemAccess {
            exclusive: false,
            ..Default::default()
        }
    }

    unsafe fn fetch<'w, 's>(
        state: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        Commands {
            queue: state.get(),
            world: unsafe { world.world() },
        }
    }

    #[inline(never)]
    fn apply(state: &mut Self::State, world: &mut World) {
        world.entities_mut().flush();
        state.get().execute(world);
    }
}

unsafe impl SystemParam for &mut World {
    type State = ();
    type Item<'w, 's> = &'w mut World;

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
        unsafe { world.world_mut() }
    }

    fn can_run(_world: &World) -> bool {
        true
    }
}
