use super::World;
use petgraph::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SystemId(u32);

impl SystemId {
    pub fn index(self) -> u32 {
        self.0
    }
}

impl From<NodeIndex> for SystemId {
    fn from(id: NodeIndex) -> Self {
        Self(id.index() as u32)
    }
}

impl From<SystemId> for NodeIndex {
    fn from(id: SystemId) -> Self {
        Self::new(id.0 as usize)
    }
}

pub trait System: Send + Sync {
    fn run(&self, world: &World);
    fn components_read(&self) -> Vec<u64>;
    fn components_written(&self) -> Vec<u64>;
}

pub trait StartupSystem: System {}

pub struct SystemNode {
    pub id: SystemId,
    pub system: Box<dyn System>,
}

#[derive(Default)]
pub struct SystemGraph {
    graph: StableDiGraph<SystemNode, ()>,
}

impl SystemGraph {
    pub fn add_system(&mut self, system: Box<dyn System>) -> SystemId {
        let index = self.graph.add_node(SystemNode {
            system,
            id: SystemId(0),
        });
        self.graph[index].id = SystemId(index.index() as u32);
        self.graph[index].id
    }

    pub fn add_system_after(&mut self, system: Box<dyn System>, after: SystemId) -> SystemId {
        let index = self.graph.add_node(SystemNode {
            system,
            id: SystemId(0),
        });
        self.graph[index].id = SystemId(index.index() as u32);
        self.graph.add_edge(after.into(), index, ());
        self.graph[index].id
    }

    pub fn add_system_before(&mut self, system: Box<dyn System>, before: SystemId) -> SystemId {
        let index = self.graph.add_node(SystemNode {
            system,
            id: SystemId(0),
        });
        self.graph[index].id = SystemId(index.index() as u32);
        self.graph.add_edge(index, before.into(), ());
        self.graph[index].id
    }

    pub fn add_dependency(&mut self, dependency: SystemId, dependent: SystemId) {
        self.graph.add_edge(dependency.into(), dependent.into(), ());
    }

    pub fn run(&self, world: &World) {
        if self.graph.node_count() == 0 {
            return;
        }

        let starts: Vec<_> = self.graph.externals(Direction::Incoming).collect();
        let mut bfs = Bfs::new(&self.graph, starts[0]);
        for &start in &starts[1..] {
            bfs.stack.push_back(start);
        }

        while let Some(node) = bfs.next(&self.graph) {
            let system = &self.graph[node].system;
            system.run(world);
        }
    }
}
