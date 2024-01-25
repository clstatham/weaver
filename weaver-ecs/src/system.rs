use std::{fmt::Debug, sync::Arc};

use crate::{
    registry::{DynamicId, Registry},
    script::{interp::BuildOnWorld, Script},
};

use super::world::World;
use parking_lot::RwLock;
use petgraph::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum SystemStage {
    Startup,

    PreUpdate,

    #[default]
    Update,

    PostUpdate,

    Shutdown,
}

pub trait System: Send + Sync + 'static {
    fn run(&self, world: Arc<RwLock<World>>) -> anyhow::Result<()>;
    fn components_read(&self, registry: &Registry) -> Vec<DynamicId>;
    fn components_written(&self, registry: &Registry) -> Vec<DynamicId>;
    fn resources_read(&self, registry: &Registry) -> Vec<DynamicId>;
    fn resources_written(&self, registry: &Registry) -> Vec<DynamicId>;
    fn is_exclusive(&self) -> bool;
}

pub enum RunFn {
    Static(Box<dyn Fn(Arc<RwLock<World>>) -> anyhow::Result<()> + Send + Sync>),
    Dynamic(Box<dyn Fn() -> anyhow::Result<()> + Send + Sync>),
}

pub struct DynamicSystem {
    pub run_fn: RunFn,
    pub name: String,
    pub components_read: Vec<DynamicId>,
    pub components_written: Vec<DynamicId>,
    pub resources_read: Vec<DynamicId>,
    pub resources_written: Vec<DynamicId>,
}

impl DynamicSystem {
    pub fn new<
        F: Fn() -> anyhow::Result<()> + Send + Sync + 'static,
        CR: IntoIterator<Item = DynamicId>,
        CW: IntoIterator<Item = DynamicId>,
        RR: IntoIterator<Item = DynamicId>,
        RW: IntoIterator<Item = DynamicId>,
    >(
        name: impl Into<String>,
        components_read: CR,
        components_written: CW,
        resources_read: RR,
        resources_written: RW,
        run_fn: F,
    ) -> Self {
        Self {
            name: name.into(),
            run_fn: RunFn::Dynamic(Box::new(run_fn)),
            components_read: components_read.into_iter().collect(),
            components_written: components_written.into_iter().collect(),
            resources_read: resources_read.into_iter().collect(),
            resources_written: resources_written.into_iter().collect(),
        }
    }

    pub fn from_system<T: System>(system: T, registry: &Registry) -> Self {
        Self {
            name: std::any::type_name::<T>().to_string(),
            components_read: system.components_read(registry),
            components_written: system.components_written(registry),
            resources_read: system.resources_read(registry),
            resources_written: system.resources_written(registry),
            run_fn: RunFn::Static(Box::new(move |world| system.run(world))),
        }
    }

    pub fn builder(name: &str) -> DynamicSystemBuilder {
        DynamicSystemBuilder::new(name)
    }

    pub fn script_builder(name: &str) -> ScriptSystemBuilder {
        ScriptSystemBuilder::new(name)
    }

    pub fn load_script(
        path: impl AsRef<std::path::Path>,
        world: Arc<RwLock<World>>,
    ) -> anyhow::Result<()> {
        let script = Script::load(path)?;
        script.build(world)?;
        Ok(())
    }
}

impl System for DynamicSystem {
    fn run(&self, world: Arc<RwLock<World>>) -> anyhow::Result<()> {
        match &self.run_fn {
            RunFn::Static(run_fn) => (run_fn)(world),
            RunFn::Dynamic(run_fn) => (run_fn)(),
        }
    }

    fn components_read(&self, _registry: &Registry) -> Vec<DynamicId> {
        self.components_read.clone()
    }

    fn components_written(&self, _registry: &Registry) -> Vec<DynamicId> {
        self.components_written.clone()
    }

    fn resources_read(&self, _registry: &Registry) -> Vec<DynamicId> {
        self.resources_read.clone()
    }

    fn resources_written(&self, _registry: &Registry) -> Vec<DynamicId> {
        self.resources_written.clone()
    }

    fn is_exclusive(&self) -> bool {
        false
    }
}

pub struct DynamicSystemBuilder {
    name: String,
    components_read: Vec<DynamicId>,
    components_written: Vec<DynamicId>,
    resources_read: Vec<DynamicId>,
    resources_written: Vec<DynamicId>,
}

impl DynamicSystemBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            components_read: Vec::new(),
            components_written: Vec::new(),
            resources_read: Vec::new(),
            resources_written: Vec::new(),
        }
    }

    pub fn read_component(&mut self, components: &[DynamicId]) -> &mut Self {
        self.components_read.extend_from_slice(components);
        self
    }

    pub fn write_component(&mut self, components: &[DynamicId]) -> &mut Self {
        self.components_written.extend_from_slice(components);
        self
    }

    pub fn read_resource(&mut self, resources: &[DynamicId]) -> &mut Self {
        self.resources_read.extend_from_slice(resources);
        self
    }

    pub fn write_resource(&mut self, resources: &[DynamicId]) -> &mut Self {
        self.resources_written.extend_from_slice(resources);
        self
    }

    pub fn build<F>(self, run: F) -> DynamicSystem
    where
        F: Fn() -> anyhow::Result<()> + Send + Sync + 'static,
    {
        DynamicSystem::new(
            self.name,
            self.components_read,
            self.components_written,
            self.resources_read,
            self.resources_written,
            run,
        )
    }
}

pub struct ScriptSystemBuilder {
    name: String,
}

impl ScriptSystemBuilder {
    pub fn new(name: &str) -> Self {
        Self { name: name.into() }
    }

    #[must_use]
    pub fn build<F>(self, run_fn: F) -> DynamicSystem
    where
        F: Fn() -> anyhow::Result<()> + Send + Sync + 'static,
    {
        let components_read = Vec::new();
        let components_written = Vec::new();
        let resources_read = Vec::new();
        let resources_written = Vec::new();

        // todo: add a way to specify which components and resources are read and written by the script
        // this is necessary to run scripts in parallel

        DynamicSystem::new(
            self.name,
            components_read,
            components_written,
            resources_read,
            resources_written,
            run_fn,
        )
    }
}

pub struct SystemNode {
    pub id: NodeIndex,
    pub system: Arc<DynamicSystem>,
}

#[derive(Default)]
pub struct SystemGraph {
    graph: StableDiGraph<SystemNode, ()>,
}

impl SystemGraph {
    pub fn has_system(&self, id: NodeIndex) -> bool {
        self.graph.contains_node(id)
    }

    pub fn add_dynamic_system(&mut self, system: DynamicSystem) -> NodeIndex {
        let index = self.graph.add_node(SystemNode {
            id: NodeIndex::default(),
            system: Arc::new(system),
        });
        self.graph[index].id = index;
        self.graph[index].id
    }

    pub fn add_system<T: System>(&mut self, system: T, registry: &Registry) -> NodeIndex {
        let index = self.graph.add_node(SystemNode {
            id: NodeIndex::default(),
            system: Arc::new(DynamicSystem::from_system(system, registry)),
        });
        self.graph[index].id = index;
        self.graph[index].id
    }

    pub fn add_system_after<T: System>(
        &mut self,
        system: T,
        after: NodeIndex,
        registry: &Registry,
    ) -> NodeIndex {
        let index = self.graph.add_node(SystemNode {
            id: NodeIndex::default(),
            system: Arc::new(DynamicSystem::from_system(system, registry)),
        });
        self.graph[index].id = index;
        self.graph.add_edge(after, index, ());
        self.graph[index].id
    }

    pub fn add_system_before<T: System>(
        &mut self,
        system: T,
        before: NodeIndex,
        registry: &Registry,
    ) -> NodeIndex {
        let index = self.graph.add_node(SystemNode {
            id: NodeIndex::default(),
            system: Arc::new(DynamicSystem::from_system(system, registry)),
        });
        self.graph[index].id = index;
        self.graph.add_edge(index, before, ());
        self.graph[index].id
    }

    pub fn add_dependency(&mut self, dependency: DynamicId, dependent: DynamicId) {
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
                    self.add_dependency(
                        writers[i].index() as DynamicId,
                        writers[i + 1].index() as DynamicId,
                    );
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
                        self.add_dependency(
                            writer.index() as DynamicId,
                            reader.index() as DynamicId,
                        );
                    }
                }
            }
        }

        // let dot = petgraph::dot::Dot::with_config(&self.graph, &[]);
        // std::fs::write("system_graph.dot", format!("{:?}", dot))?;

        Ok(())
    }

    pub fn detect_cycles(&self) -> Option<Vec<NodeIndex>> {
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
        path: &mut Vec<NodeIndex>,
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
}
