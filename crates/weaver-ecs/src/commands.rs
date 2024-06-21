use crate::prelude::{SystemAccess, SystemParam, World, WorldLock};
use weaver_util::lock::SharedLock;

pub trait Command: Send + Sync + 'static {
    fn execute(self: Box<Self>, world: &mut World);
}

impl<F> Command for F
where
    F: FnOnce(&mut World) + Send + Sync + 'static,
{
    fn execute(self: Box<Self>, world: &mut World) {
        self(world);
    }
}

#[derive(Default)]
pub struct Commands {
    commands: SharedLock<Vec<Box<dyn Command>>>,
}

impl Commands {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<C>(&self, command: C)
    where
        C: Command,
    {
        self.commands.write().push(Box::new(command));
    }

    pub fn execute(&self, world: &mut World) {
        for command in self.commands.write().drain(..) {
            command.execute(world);
        }
    }
}

impl SystemParam for Commands {
    type State = SharedLock<Vec<Box<dyn Command>>>;
    type Fetch<'w, 's> = Self;

    fn access() -> crate::prelude::SystemAccess {
        SystemAccess {
            resources_read: Vec::new(),
            resources_written: Vec::new(),
            components_read: Vec::new(),
            components_written: Vec::new(),
            exclusive: false,
        }
    }

    fn init_state(_world: &WorldLock) -> Self::State {
        SharedLock::new(Vec::new())
    }

    fn fetch<'w, 's>(state: &'s mut Self::State, _world: &WorldLock) -> Self::Fetch<'w, 's> {
        Self {
            commands: state.clone(),
        }
    }

    fn apply_deferred_mutations(state: &mut Self::State, world: &mut World) {
        let commands = Commands {
            commands: state.clone(),
        };
        commands.execute(world);
    }
}
