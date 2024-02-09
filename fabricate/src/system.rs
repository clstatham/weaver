use std::sync::Arc;

use crate::prelude::{Data, Entity, Graph, LockedWorldHandle};

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
pub trait System {
    fn run(&self, world: LockedWorldHandle, inputs: &[Data]) -> anyhow::Result<Vec<Data>>;
    fn reads(&self) -> Vec<Entity>;
    fn writes(&self) -> Vec<Entity>;
}

#[derive(Clone)]
pub struct DynamicSystem {
    #[allow(clippy::type_complexity)]
    run: Arc<
        dyn for<'a> Fn(LockedWorldHandle, &'a [Data]) -> anyhow::Result<Vec<Data>>
            + Send
            + Sync
            + 'static,
    >,
}

impl DynamicSystem {
    pub fn new(
        run: impl for<'a> Fn(LockedWorldHandle, &'a [Data]) -> anyhow::Result<Vec<Data>>
            + Send
            + Sync
            + 'static,
    ) -> Self {
        Self { run: Arc::new(run) }
    }
}

impl System for DynamicSystem {
    fn run(&self, world: LockedWorldHandle, inputs: &[Data]) -> anyhow::Result<Vec<Data>> {
        (self.run)(world, inputs)
    }

    fn reads(&self) -> Vec<Entity> {
        vec![] // todo
    }

    fn writes(&self) -> Vec<Entity> {
        vec![] // todo
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

    pub fn run(&self, world: LockedWorldHandle) {
        let orphans = self.graph.orphans();
        for node in self.graph.bfs(orphans).unwrap() {
            let world_lock = world.read();
            let system = world_lock.get_system(*node).unwrap().clone();
            drop(world_lock);
            system.run(world.clone(), &[]).unwrap();
        }
    }
}
