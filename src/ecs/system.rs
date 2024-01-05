use super::{EcsError, World};
use petgraph::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SystemId(u32);

impl SystemId {
    pub const PLACEHOLDER: Self = Self(u32::MAX);

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
    fn run(&self, world: &World) -> anyhow::Result<()>;
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
    pub fn has_system(&self, id: SystemId) -> bool {
        self.graph.contains_node(id.into())
    }

    pub fn add_system(&mut self, system: Box<dyn System>) -> SystemId {
        let index = self.graph.add_node(SystemNode {
            system,
            id: SystemId::PLACEHOLDER,
        });
        self.graph[index].id = SystemId(index.index() as u32);
        self.graph[index].id
    }

    pub fn add_system_after(&mut self, system: Box<dyn System>, after: SystemId) -> SystemId {
        let index = self.graph.add_node(SystemNode {
            system,
            id: SystemId::PLACEHOLDER,
        });
        self.graph[index].id = SystemId(index.index() as u32);
        self.graph.add_edge(after.into(), index, ());
        self.graph[index].id
    }

    pub fn add_system_before(&mut self, system: Box<dyn System>, before: SystemId) -> SystemId {
        let index = self.graph.add_node(SystemNode {
            system,
            id: SystemId::PLACEHOLDER,
        });
        self.graph[index].id = SystemId(index.index() as u32);
        self.graph.add_edge(index, before.into(), ());
        self.graph[index].id
    }

    pub fn add_dependency(&mut self, dependency: SystemId, dependent: SystemId) {
        self.graph.add_edge(dependency.into(), dependent.into(), ());
    }

    pub fn fix_parallel_writes(&mut self) {
        let mut components_written = FxHashMap::default();
        for node in self.graph.node_indices() {
            let system = &self.graph[node].system;
            for component in system.components_written() {
                components_written
                    .entry(component)
                    .or_insert_with(FxHashSet::default)
                    .insert(node);
            }
        }

        for node in self.graph.node_indices().collect::<Vec<_>>() {
            for component in self.graph[node].system.components_written() {
                for &other in components_written[&component].iter() {
                    if node != other && !self.graph.contains_edge(node, other) {
                        self.add_dependency(other.into(), node.into());
                    }
                }
            }
        }
    }

    pub fn detect_cycles(&self) -> Option<Vec<SystemId>> {
        let mut visited = FxHashSet::default();
        let mut stack = Vec::new();
        let mut path = Vec::new();

        for node in self.graph.node_indices() {
            if !visited.contains(&node)
                && self.detect_cycles_helper(node, &mut visited, &mut stack, &mut path)
            {
                return Some(path);
            }
        }

        None
    }

    fn detect_cycles_helper(
        &self,
        node: NodeIndex,
        visited: &mut FxHashSet<NodeIndex>,
        stack: &mut Vec<NodeIndex>,
        path: &mut Vec<SystemId>,
    ) -> bool {
        visited.insert(node);
        stack.push(node);
        path.push(self.graph[node].id);

        for neighbor in self.graph.neighbors(node) {
            if !visited.contains(&neighbor) {
                if self.detect_cycles_helper(neighbor, visited, stack, path) {
                    return true;
                }
            } else if stack.contains(&neighbor) {
                path.push(self.graph[neighbor].id);
                return true;
            }
        }

        stack.pop();
        false
    }

    pub fn run(&self, world: &World) -> anyhow::Result<()> {
        if self.graph.node_count() == 0 {
            return Ok(());
        }

        if self.detect_cycles().is_some() {
            return Err(EcsError::SystemDependencyCycleDetected.into());
        }

        let starts: Vec<_> = self.graph.externals(Direction::Incoming).collect();
        let mut bfs = Bfs::new(&self.graph, starts[0]);
        for &start in &starts[1..] {
            bfs.stack.push_back(start);
        }

        while let Some(node) = bfs.next(&self.graph) {
            let system = &self.graph[node].system;
            system.run(world)?;
        }

        Ok(())
    }
}
