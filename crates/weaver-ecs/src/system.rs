use std::sync::Arc;

use crate::prelude::World;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemStage {
    PreInit,
    Init,
    PostInit,

    PreUpdate,
    Update,
    PostUpdate,

    Ui,

    PreRender,
    Render,
    PostRender,

    PreShutdown,
    Shutdown,
    PostShutdown,
}

pub trait System: Send + Sync + 'static {
    fn run(&self, world: &World) -> anyhow::Result<()>;
}

impl<T> System for T
where
    T: Fn(&World) -> anyhow::Result<()> + Send + Sync + 'static,
{
    fn run(&self, world: &World) -> anyhow::Result<()> {
        self(world)
    }
}

impl System for Arc<dyn System> {
    fn run(&self, world: &World) -> anyhow::Result<()> {
        self.as_ref().run(world)
    }
}
