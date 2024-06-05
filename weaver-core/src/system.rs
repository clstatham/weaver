use std::sync::Arc;

use crate::prelude::Scene;

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
    fn run(&self, scene: &Scene) -> anyhow::Result<()>;
}

impl<T> System for T
where
    T: Fn(&Scene) -> anyhow::Result<()> + Send + Sync + 'static,
{
    fn run(&self, scene: &Scene) -> anyhow::Result<()> {
        self(scene)
    }
}

impl System for Arc<dyn System> {
    fn run(&self, scene: &Scene) -> anyhow::Result<()> {
        self.as_ref().run(scene)
    }
}
