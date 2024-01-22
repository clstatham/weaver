use std::{collections::VecDeque, fmt::Debug, ops::Deref, sync::Arc};

use crate::{
    id::{DynamicId, Registry},
    prelude::Commands,
    query::{DynamicQuery, DynamicQueryParam, DynamicQueryParams},
    resource::{DynRes, DynResMut},
    script::interp::BuildOnWorld,
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
    ) -> anyhow::Result<Vec<Self>> {
        let mut script = crate::script::Script::load(path)?;
        script.parse();
        let systems = script.build(world)?;
        Ok(systems)
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

pub enum ScriptBuilderParamType {
    Query(DynamicQueryParams),
    Res(DynamicId),
    ResMut(DynamicId),
    Commands,
}

pub struct ScriptBuilderParam {
    pub name: String,
    pub param: ScriptBuilderParamType,
}

pub enum ScriptParamType<'a> {
    Query(DynamicQuery<'a>),
    Res(DynRes<'a>),
    ResMut(DynResMut<'a>),
    Commands(Commands),
}

pub struct ScriptParam<'a> {
    pub name: String,
    pub param: ScriptParamType<'a>,
}

impl<'a> Deref for ScriptParam<'a> {
    type Target = ScriptParamType<'a>;

    fn deref(&self) -> &Self::Target {
        &self.param
    }
}

impl<'a> ScriptParamType<'a> {
    pub fn unwrap_query(&self) -> &DynamicQuery<'a> {
        match self {
            ScriptParamType::Query(query) => query,
            _ => panic!("Expected ScriptParams::Query"),
        }
    }

    pub fn unwrap_res(&self) -> &DynRes<'a> {
        match self {
            ScriptParamType::Res(res) => res,
            _ => panic!("Expected ScriptParams::Res"),
        }
    }

    pub fn unwrap_res_mut(&self) -> &DynResMut<'a> {
        match self {
            ScriptParamType::ResMut(res) => res,
            _ => panic!("Expected ScriptParams::ResMut"),
        }
    }

    pub fn unwrap_commands(&self) -> &Commands {
        match self {
            ScriptParamType::Commands(commands) => commands,
            _ => panic!("Expected ScriptParams::Commands"),
        }
    }
}

pub struct ScriptSystemBuilder {
    name: String,
    params: Vec<ScriptBuilderParam>,
}

impl ScriptSystemBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            params: Vec::new(),
        }
    }

    #[must_use]
    pub fn query(mut self, name: &str, query: DynamicQueryParams) -> Self {
        self.params.push(ScriptBuilderParam {
            name: name.into(),
            param: ScriptBuilderParamType::Query(query),
        });
        self
    }

    #[must_use]
    pub fn res(mut self, name: &str, resource: DynamicId) -> Self {
        self.params.push(ScriptBuilderParam {
            name: name.into(),
            param: ScriptBuilderParamType::Res(resource),
        });
        self
    }

    #[must_use]
    pub fn res_mut(mut self, name: &str, resource: DynamicId) -> Self {
        self.params.push(ScriptBuilderParam {
            name: name.into(),
            param: ScriptBuilderParamType::ResMut(resource),
        });
        self
    }

    #[must_use]
    pub fn commands(mut self) -> Self {
        self.params.push(ScriptBuilderParam {
            name: "commands".into(),
            param: ScriptBuilderParamType::Commands,
        });
        self
    }

    #[must_use]
    pub fn build<S>(self, world: Arc<RwLock<World>>, script: S) -> DynamicSystem
    where
        S: Fn(&[ScriptParam]) -> anyhow::Result<()> + Send + Sync + 'static,
    {
        let mut components_read = Vec::new();
        let mut components_written = Vec::new();
        let mut resources_read = Vec::new();
        let mut resources_written = Vec::new();

        for param in self.params.iter() {
            match &param.param {
                ScriptBuilderParamType::Query(query) => {
                    for param in query.iter() {
                        match param {
                            DynamicQueryParam::Read(component) => {
                                components_read.push(*component);
                            }
                            DynamicQueryParam::Write(component) => {
                                components_written.push(*component);
                            }
                            _ => {}
                        }
                    }
                }
                ScriptBuilderParamType::Res(res) => {
                    resources_read.push(*res);
                }
                ScriptBuilderParamType::ResMut(res) => {
                    resources_written.push(*res);
                }
                ScriptBuilderParamType::Commands => {}
            }
        }

        let world_clone = world.clone();
        let run_fn = move || {
            let world_lock = world_clone.read();
            let mut params = Vec::new();
            let mut has_commands = false;

            for param in self.params.iter() {
                match &param.param {
                    ScriptBuilderParamType::Query(query) => {
                        params.push(ScriptParam {
                            name: param.name.clone(),
                            param: ScriptParamType::Query(DynamicQuery::new(
                                &world_lock.components,
                                query.clone(),
                            )),
                        });
                    }
                    ScriptBuilderParamType::Res(res) => {
                        params.push(ScriptParam {
                            name: param.name.clone(),
                            param: ScriptParamType::Res(DynRes::new(
                                world_lock.resources.get(res).unwrap().read(),
                            )),
                        });
                    }
                    ScriptBuilderParamType::ResMut(res) => {
                        params.push(ScriptParam {
                            name: param.name.clone(),
                            param: ScriptParamType::ResMut(DynResMut::new(
                                world_lock.resources.get(res).unwrap().write(),
                            )),
                        });
                    }
                    ScriptBuilderParamType::Commands => {
                        has_commands = true;
                        params.push(ScriptParam {
                            name: param.name.clone(),
                            param: ScriptParamType::Commands(Commands::new(&world_lock)),
                        });
                    }
                }
            }

            (script)(&params)?;

            if has_commands {
                let commands = params
                    .drain(..)
                    .find_map(|param| match param.param {
                        ScriptParamType::Commands(commands) => Some(commands),
                        _ => None,
                    })
                    .unwrap();
                drop(params);
                drop(world_lock);
                let mut world_lock = world_clone.write();
                commands.finalize(&mut world_lock);
            }

            Ok(())
        };

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
