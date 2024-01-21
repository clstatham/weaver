use std::{collections::VecDeque, fmt::Debug, sync::Arc};

use crate::id::{DynamicId, Registry};

use super::world::World;
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum SystemStage {
    Startup,

    PreUpdate,

    #[default]
    Update,

    PostUpdate,

    Shutdown,
}

pub trait System: Send + Sync {
    fn run(&self, world: Arc<RwLock<World>>) -> anyhow::Result<()>;
    fn components_read(&self, registry: &Registry) -> Vec<DynamicId>;
    fn components_written(&self, registry: &Registry) -> Vec<DynamicId>;
    fn resources_read(&self, registry: &Registry) -> Vec<DynamicId>;
    fn resources_written(&self, registry: &Registry) -> Vec<DynamicId>;
    fn is_exclusive(&self) -> bool;
}

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
            id: SystemId::PLACEHOLDER,
            system,
        });
        self.graph[index].id = SystemId(index.index() as u32);
        self.graph[index].id
    }

    pub fn add_system_after(&mut self, system: Arc<dyn System>, after: SystemId) -> SystemId {
        let index = self.graph.add_node(SystemNode {
            id: SystemId::PLACEHOLDER,
            system,
        });
        self.graph[index].id = SystemId(index.index() as u32);
        self.graph.add_edge(after.into(), index, ());
        self.graph[index].id
    }

    pub fn add_system_before(&mut self, system: Arc<dyn System>, before: SystemId) -> SystemId {
        let index = self.graph.add_node(SystemNode {
            id: SystemId::PLACEHOLDER,
            system,
        });
        self.graph[index].id = SystemId(index.index() as u32);
        self.graph.add_edge(index, before.into(), ());
        self.graph[index].id
    }

    pub fn add_dependency(&mut self, dependency: SystemId, dependent: SystemId) {
        self.graph.add_edge(dependency.into(), dependent.into(), ());
    }

    pub fn autodetect_dependencies(&mut self, registry: &Registry) -> anyhow::Result<()> {
        let mut components_read = FxHashMap::default();
        let mut components_written = FxHashMap::default();

        for node in self.graph.node_indices() {
            let system = &self.graph[node].system;
            for component in system.components_read(registry) {
                components_read
                    .entry(component)
                    .or_insert_with(Vec::new)
                    .push(node);
            }
            for component in system.components_written(registry) {
                components_written
                    .entry(component)
                    .or_insert_with(Vec::new)
                    .push(node);
            }
        }

        // components use RwLocks, so they can be read by multiple systems at once, but only written to by one system at a time

        // if there are any two systems that write to the same component, add a dependency from the first system to the second
        for (_, writers) in components_written.iter() {
            if writers.len() > 1 {
                for i in 0..writers.len() - 1 {
                    // don't create dependency cycles
                    if writers[i] == writers[i + 1] {
                        continue;
                    }
                    // don't add duplicate dependencies
                    if self.graph.contains_edge(writers[i], writers[i + 1]) {
                        continue;
                    }
                    self.add_dependency(writers[i].into(), writers[i + 1].into());
                }
            }
        }

        // add a dependency from each system that writes to a component to each system that reads from that component
        for (&component, writers) in components_written.iter() {
            for &writer in writers {
                if let Some(readers) = components_read.get(&component) {
                    for &reader in readers {
                        // don't create dependency cycles
                        if writer == reader {
                            continue;
                        }
                        // don't add duplicate dependencies
                        if self.graph.contains_edge(writer, reader) {
                            continue;
                        }
                        self.add_dependency(writer.into(), reader.into());
                    }
                }
            }
        }

        // let dot = petgraph::dot::Dot::with_config(&self.graph, &[]);
        // std::fs::write("system_graph.dot", format!("{:?}", dot))?;

        Ok(())
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

    pub fn run(&self, world: &Arc<RwLock<World>>) -> anyhow::Result<()> {
        if self.graph.node_count() == 0 {
            return Ok(());
        }

        if self.detect_cycles().is_some() {
            return Err(anyhow::anyhow!("System dependency cycle detected"));
        }

        let starts: Vec<_> = self.graph.externals(Direction::Incoming).collect();
        let mut bfs = Bfs::new(&self.graph, starts[0]);
        for &start in &starts[1..] {
            bfs.stack.push_back(start);
        }

        while let Some(node) = bfs.next(&self.graph) {
            let system = &self.graph[node].system;
            system.run(world.clone())?;
        }

        Ok(())
    }

    pub fn run_parallel(&self, world: &Arc<RwLock<World>>) -> anyhow::Result<()> {
        if self.graph.node_count() == 0 {
            return Ok(());
        }

        if self.detect_cycles().is_some() {
            return Err(anyhow::anyhow!("System dependency cycle detected"));
        }

        let mut systems_running = FxHashMap::default();

        let mut systems_finished = FxHashSet::default();

        // add all systems with no dependencies to the queue
        let mut queue = VecDeque::new();
        for node in self.graph.externals(Direction::Incoming) {
            queue.push_back(node);
        }

        if queue.is_empty() {
            return Err(anyhow::anyhow!(
                "System dependency cycle detected (no systems without dependencies)"
            ));
        }

        if queue.len() == self.graph.node_count() {
            // all systems have no dependencies
            // run them all!
            for node in queue {
                let world = world.clone();
                let system = self.graph[node].system.clone();

                let (tx, rx) = crossbeam_channel::bounded(1);
                rayon::spawn(move || {
                    system.run(world.clone()).unwrap();
                    let _ = tx.send(()); // notify that the system has finished running
                });

                systems_running.insert(node, rx);
            }

            // wait for all systems to finish running
            loop {
                if systems_running.is_empty() {
                    break;
                }

                for (&node, rx) in systems_running.iter_mut() {
                    if let Ok(()) = rx.try_recv() {
                        // system finished running
                        systems_finished.insert(node);
                    }
                }

                // remove systems that have finished running
                for node in systems_finished.iter() {
                    systems_running.remove(node);
                }

                rayon::yield_now();
            }

            return Ok(());
        }

        // some systems have dependencies

        loop {
            // check if all systems have finished running
            if systems_finished.len() == self.graph.node_count() {
                break;
            }
            // check if there are any systems that can be run
            if let Some(node) = queue.pop_front() {
                if !systems_running.contains_key(&node) && !systems_finished.contains(&node) {
                    let system = &self.graph[node].system.clone();

                    let system = system.clone();
                    let world = world.clone();
                    let (tx, rx) = crossbeam_channel::bounded(1);
                    rayon::spawn(move || {
                        system.run(world.clone()).unwrap();
                        let _ = tx.send(()); // notify that the system has finished running
                    });

                    systems_running.insert(node, rx);
                }
            }

            // check if any systems have finished running
            for (&node, rx) in systems_running.iter_mut() {
                if let Ok(()) = rx.try_recv() {
                    // system finished running
                    systems_finished.insert(node);
                }
            }

            // remove systems that have finished running
            for node in systems_finished.iter() {
                systems_running.remove(node);
            }

            // enqueue systems whose dependencies have been met
            for node in self.graph.node_indices() {
                if !systems_finished.contains(&node) && !queue.contains(&node) {
                    let mut dependencies_met = true;
                    for neighbor in self.graph.neighbors_directed(node, Direction::Incoming) {
                        if !systems_finished.contains(&neighbor) {
                            dependencies_met = false;
                            break;
                        }
                    }
                    if dependencies_met {
                        queue.push_back(node);
                    }
                }
            }

            rayon::yield_now();
        }

        Ok(())
    }
}
