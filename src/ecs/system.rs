use std::{collections::VecDeque, sync::Arc};

use super::{EcsError, World};
use parking_lot::RwLock;
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
    pub system: Arc<dyn System>,
}

#[derive(Default)]
pub struct SystemGraph {
    graph: StableDiGraph<SystemNode, ()>,
}

impl SystemGraph {
    pub fn has_system(&self, id: SystemId) -> bool {
        self.graph.contains_node(id.into())
    }

    pub fn add_system(&mut self, system: Arc<dyn System>) -> SystemId {
        let index = self.graph.add_node(SystemNode {
            system,
            id: SystemId::PLACEHOLDER,
        });
        self.graph[index].id = SystemId(index.index() as u32);
        self.graph[index].id
    }

    pub fn add_system_after(&mut self, system: Arc<dyn System>, after: SystemId) -> SystemId {
        let index = self.graph.add_node(SystemNode {
            system,
            id: SystemId::PLACEHOLDER,
        });
        self.graph[index].id = SystemId(index.index() as u32);
        self.graph.add_edge(after.into(), index, ());
        self.graph[index].id
    }

    pub fn add_system_before(&mut self, system: Arc<dyn System>, before: SystemId) -> SystemId {
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

    pub fn run_parallel(&self, world: &Arc<RwLock<World>>) -> anyhow::Result<()> {
        if self.graph.node_count() == 0 {
            return Ok(());
        }

        if self.detect_cycles().is_some() {
            return Err(EcsError::SystemDependencyCycleDetected.into());
        }

        let mut systems_running = FxHashMap::default();

        let mut systems_run = FxHashSet::default();

        // add all systems with no dependencies to the queue
        let mut queue = VecDeque::new();
        for node in self.graph.externals(Direction::Incoming) {
            queue.push_back(node);
        }

        if queue.is_empty() {
            return Err(EcsError::SystemDependencyCycleDetected.into());
        }

        if queue.len() == self.graph.node_count() {
            // all systems have no dependencies
            // run them all!
            for node in queue {
                let world = world.clone();
                let system = self.graph[node].system.clone();

                let handle = std::thread::spawn(move || {
                    system.run(&world.read()).unwrap();
                });

                systems_running.insert(node, handle);
            }

            // wait for all systems to finish running
            loop {
                if systems_running.is_empty() {
                    break;
                }

                for (&node, rx) in systems_running.iter_mut() {
                    if rx.is_finished() {
                        // system finished running
                        systems_run.insert(node);
                    }
                }

                // remove systems that have finished running
                for node in systems_run.iter() {
                    systems_running.remove(node);
                }

                std::thread::yield_now();
            }

            return Ok(());
        }

        // some systems have dependencies

        loop {
            // check if all systems have finished running
            if systems_run.len() == self.graph.node_count() {
                break;
            }
            // check if there are any systems that can be run
            if let Some(node) = queue.pop_front() {
                if !systems_running.contains_key(&node) && !systems_run.contains(&node) {
                    let system = &self.graph[node].system.clone();

                    let system = system.clone();
                    let world = world.clone();
                    let handle = std::thread::spawn(move || {
                        system.run(&world.read()).unwrap();
                    });

                    systems_running.insert(node, handle);
                }
            }

            // check if any systems have finished running
            for (&node, rx) in systems_running.iter_mut() {
                if rx.is_finished() {
                    // system finished running
                    systems_run.insert(node);
                }
            }

            // remove systems that have finished running
            for node in systems_run.iter() {
                systems_running.remove(node);
            }

            // enqueue systems whose dependencies have been met
            for node in self.graph.node_indices() {
                if !systems_run.contains(&node) && !queue.contains(&node) {
                    let mut dependencies_met = true;
                    for neighbor in self.graph.neighbors_directed(node, Direction::Incoming) {
                        if !systems_run.contains(&neighbor) {
                            dependencies_met = false;
                            break;
                        }
                    }
                    if dependencies_met {
                        queue.push_back(node);
                    }
                }
            }

            std::thread::yield_now();
        }

        Ok(())
    }
}
