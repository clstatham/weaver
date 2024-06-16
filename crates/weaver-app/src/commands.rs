use weaver_ecs::prelude::World;
use weaver_util::prelude::Result;

pub trait Command: 'static {
    fn execute(&self, world: &mut World) -> Result<()>;
}

impl<T> Command for T
where
    T: Fn(&mut World) -> Result<()> + Send + Sync + 'static,
{
    fn execute(&self, world: &mut World) -> Result<()> {
        self(world)
    }
}

#[derive(Default)]
pub struct Commands {
    commands: Vec<Box<dyn Command>>,
}

impl Commands {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<T>(&mut self, command: T)
    where
        T: Command,
    {
        self.commands.push(Box::new(command));
    }

    pub fn execute(&self, world: &mut World) -> Result<()> {
        for command in &self.commands {
            command.execute(world)?;
        }

        Ok(())
    }
}
