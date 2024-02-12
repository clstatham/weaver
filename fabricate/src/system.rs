use std::sync::Arc;

use crate::{self as fabricate, commands::Commands, world::World};
use fabricate_macro::Component;

use crate::prelude::{Entity, Graph, LockedWorldHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SystemStage {
    Startup,
    PreUpdate,
    Update,
    PostUpdate,
    Ui,
    PreRender,
    Render,
    PostRender,
    Shutdown,
}

/// A system that can be run on a [`World`].
/// Systems can read, write, and access entities and components in the [`World`].
///
/// [`World`]: crate::world::World
pub trait System: Send + Sync + 'static {
    fn run(&self, world: &World, commands: &mut Commands) -> anyhow::Result<()>;
}

impl<T> System for T
where
    T: Fn(&World, &mut Commands) -> anyhow::Result<()> + Send + Sync + 'static,
{
    fn run(&self, world: &World, commands: &mut Commands) -> anyhow::Result<()> {
        self(world, commands)
    }
}

impl System for Arc<dyn System> {
    fn run(&self, world: &World, commands: &mut Commands) -> anyhow::Result<()> {
        self.as_ref().run(world, commands)
    }
}

#[derive(Clone, Component)]
pub struct DynamicSystem {
    #[allow(clippy::type_complexity)]
    run: Arc<dyn System>,
}

impl DynamicSystem {
    pub fn new(run: impl System) -> Self {
        Self { run: Arc::new(run) }
    }
}

impl System for DynamicSystem {
    fn run(&self, world: &World, commands: &mut Commands) -> anyhow::Result<()> {
        self.run.run(world, commands)
    }
}

#[derive(Clone, Default)]
pub struct SystemGraph {
    graph: Graph<Entity, ()>,
}

impl SystemGraph {
    pub fn add_system(&mut self, system_id: Entity) {
        self.graph.add_node(system_id);
    }

    pub fn run(&self, world: LockedWorldHandle) -> anyhow::Result<()> {
        let orphans = self.graph.orphans();
        for node in self.graph.bfs(orphans).unwrap() {
            let system = world.with_system(*node, |sys| sys.clone()).unwrap();
            world.defer(|world, commands| system.run(world, commands))??;
        }
        Ok(())
    }
}
