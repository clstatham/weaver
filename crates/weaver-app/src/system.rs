use std::{any::TypeId, collections::HashSet, sync::Arc};

use petgraph::prelude::*;
use rustc_hash::FxHashMap;
use weaver_ecs::{
    component::{Res, ResMut},
    prelude::{Query, Resource, World},
    query::{QueryAccess, QueryFilter},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemStage {
    PreInit,
    Init,
    PostInit,

    PreUpdate,
    Update,
    PostUpdate,

    PreUi,
    Ui,
    PostUi,

    Extract,
    PreRender,
    Render,
    PostRender,

    PreShutdown,
    Shutdown,
    PostShutdown,
}

pub struct SystemAccess {
    pub resources_read: Vec<TypeId>,
    pub resources_written: Vec<TypeId>,
    pub components_read: Vec<TypeId>,
    pub components_written: Vec<TypeId>,
}

impl SystemAccess {
    pub fn extend(&mut self, other: SystemAccess) {
        self.resources_read.extend(other.resources_read);
        self.resources_written.extend(other.resources_written);
        self.components_read.extend(other.components_read);
        self.components_written.extend(other.components_written);
    }
}

pub trait System: 'static + Send + Sync {
    fn access(&self) -> SystemAccess;
    fn run(&self, world: Arc<World>) -> anyhow::Result<()>;
}

pub trait SystemParam {
    fn access() -> SystemAccess;
    fn fetch(world: Arc<World>) -> Option<Self>
    where
        Self: Sized;
}

impl<Q> SystemParam for Query<Q>
where
    Q: QueryFilter,
{
    fn access() -> SystemAccess {
        SystemAccess {
            resources_read: Vec::new(),
            resources_written: Vec::new(),
            components_read: Q::access()
                .iter()
                .filter_map(|(ty, access)| {
                    if let QueryAccess::ReadOnly = access {
                        Some(*ty)
                    } else {
                        None
                    }
                })
                .collect(),
            components_written: Q::access()
                .iter()
                .filter_map(|(ty, access)| {
                    if let QueryAccess::ReadWrite = access {
                        Some(*ty)
                    } else {
                        None
                    }
                })
                .collect(),
        }
    }
    fn fetch(world: Arc<World>) -> Option<Self> {
        Some(Query::new(world))
    }
}

impl<T: Resource> SystemParam for Res<T> {
    fn access() -> SystemAccess {
        SystemAccess {
            resources_read: vec![TypeId::of::<T>()],
            resources_written: Vec::new(),
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }
    fn fetch(world: Arc<World>) -> Option<Self> {
        world.get_resource::<T>()
    }
}

impl<T: Resource> SystemParam for ResMut<T> {
    fn access() -> SystemAccess {
        SystemAccess {
            resources_read: Vec::new(),
            resources_written: vec![TypeId::of::<T>()],
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }
    fn fetch(world: Arc<World>) -> Option<Self> {
        world.get_resource_mut::<T>()
    }
}

pub trait FunctionSystem<Marker>: 'static + Send + Sync {
    fn into_system(self) -> Arc<dyn System>;
}

macro_rules! impl_function_system {
    ($($param:ident),*) => {
        impl<Func, $($param),*> FunctionSystem<fn($($param),*)> for Func
        where
            Func: Fn($($param),*) -> anyhow::Result<()> + 'static + Send + Sync,
            $($param: SystemParam + 'static + Send + Sync),*
        {

            #[allow(unused_parens, non_snake_case)]
            fn into_system(self) -> Arc<dyn System> {
                struct FunctionSystemImpl<Func, $($param),*> {
                    func: Func,
                    _marker: std::marker::PhantomData<($($param),*)>,
                }

                impl <Func, $($param),*> System for FunctionSystemImpl<Func, $($param),*>
                where
                    Func: Fn($($param),*) -> anyhow::Result<()> + 'static + Send + Sync,
                    $($param: SystemParam + 'static + Send + Sync),*
                {
                    fn access(&self) -> SystemAccess {
                        let mut access = SystemAccess {
                            resources_read: Vec::new(),
                            resources_written: Vec::new(),
                            components_read: Vec::new(),
                            components_written: Vec::new(),
                        };

                        $(
                            access.extend($param::access());
                        )*

                        access
                    }

                    fn run(&self, world: Arc<World>) -> anyhow::Result<()> {
                        let ($($param),*) = ($($param::fetch(world.clone()).ok_or_else(|| anyhow::anyhow!("Failed to fetch system param"))?),*);
                        (self.func)($($param),*)
                    }
                }

                Arc::new(FunctionSystemImpl {
                    func: self,
                    _marker: std::marker::PhantomData,
                })
            }
        }
    };
}

impl_function_system!(A);
impl_function_system!(A, B);
impl_function_system!(A, B, C);
impl_function_system!(A, B, C, D);
impl_function_system!(A, B, C, D, E);
impl_function_system!(A, B, C, D, E, F);
impl_function_system!(A, B, C, D, E, F, G);
impl_function_system!(A, B, C, D, E, F, G, H);

impl<Func> FunctionSystem<()> for Func
where
    Func: Fn(Arc<World>) -> anyhow::Result<()> + 'static + Send + Sync,
{
    fn into_system(self) -> Arc<dyn System> {
        struct FunctionSystemImpl<Func> {
            func: Func,
        }

        impl<Func> System for FunctionSystemImpl<Func>
        where
            Func: Fn(Arc<World>) -> anyhow::Result<()> + 'static + Send + Sync,
        {
            fn access(&self) -> SystemAccess {
                SystemAccess {
                    resources_read: Vec::new(),
                    resources_written: Vec::new(),
                    components_read: Vec::new(),
                    components_written: Vec::new(),
                }
            }
            fn run(&self, world: Arc<World>) -> anyhow::Result<()> {
                (self.func)(world)
            }
        }

        Arc::new(FunctionSystemImpl { func: self })
    }
}

#[derive(Default)]
pub struct SystemGraph {
    systems: StableDiGraph<Arc<dyn System>, ()>,
    index_cache: FxHashMap<TypeId, NodeIndex>,
}

impl SystemGraph {
    pub fn add_system<M, S>(&mut self, system: S) -> NodeIndex
    where
        S: FunctionSystem<M>,
    {
        let node = self.systems.add_node(system.into_system());
        self.index_cache.insert(TypeId::of::<S>(), node);
        node
    }

    pub fn add_edge<M1, M2, S1, S2>(&mut self, _parent: S1, _child: S2)
    where
        S1: FunctionSystem<M1>,
        S2: FunctionSystem<M2>,
    {
        let parent = self.index_cache[&TypeId::of::<S1>()];
        let child = self.index_cache[&TypeId::of::<S2>()];
        self.systems.add_edge(parent, child, ());
    }

    pub fn add_system_after<M1, M2, S1, S2>(&mut self, system: S1, _after: S2)
    where
        S1: FunctionSystem<M1>,
        S2: FunctionSystem<M2>,
    {
        let node = self.add_system(system);
        let parent = self.index_cache[&TypeId::of::<S2>()];
        self.systems.add_edge(parent, node, ());
    }

    pub fn add_system_before<M1, M2, S1, S2>(&mut self, system: S1, _before: S2)
    where
        S1: FunctionSystem<M1>,
        S2: FunctionSystem<M2>,
    {
        let node = self.add_system(system);
        let child = self.index_cache[&TypeId::of::<S2>()];
        self.systems.add_edge(node, child, ());
    }

    pub fn run(&mut self, world: Arc<World>) -> anyhow::Result<()> {
        self.resolve_dependencies(100)?;
        let mut schedule = petgraph::visit::Topo::new(&self.systems);
        while let Some(node) = schedule.next(&self.systems) {
            let system = self.systems[node].clone();
            system.run(world.clone())?;
        }
        Ok(())
    }

    pub fn get_layers(&self) -> Vec<Vec<NodeIndex>> {
        let mut schedule = petgraph::visit::Topo::new(&self.systems);

        let mut seen = HashSet::new();
        let mut layers = Vec::new();
        let mut current_layer = Vec::new();
        while let Some(node) = schedule.next(&self.systems) {
            if seen.contains(&node) {
                continue;
            }
            seen.insert(node);
            current_layer.push(node);
            if self
                .systems
                .neighbors_directed(node, Direction::Incoming)
                .count()
                == 0
            {
                layers.push(current_layer);
                current_layer = Vec::new();
            }

            for child in self.systems.neighbors_directed(node, Direction::Outgoing) {
                if self
                    .systems
                    .neighbors_directed(child, Direction::Incoming)
                    .all(|parent| seen.contains(&parent))
                {
                    seen.insert(child);
                    current_layer.push(child);
                }
            }
        }

        layers
    }

    pub fn resolve_dependencies(&mut self, depth: usize) -> anyhow::Result<()> {
        if depth == 0 {
            return Err(anyhow::anyhow!("Cyclic system dependency detected"));
        }
        let layers = self.get_layers();

        let mut try_again = false;

        // systems that access the same resources mutably cannot run concurrently
        for layer in layers {
            for i in 0..layer.len() {
                for j in 0..i {
                    let system_i = &self.systems[layer[i]];
                    let system_j = &self.systems[layer[j]];
                    let access_i = system_i.access();
                    let access_j = system_j.access();

                    for resource_i in &access_i.resources_written {
                        if access_j.resources_read.contains(resource_i)
                            || access_j.resources_written.contains(resource_i)
                        {
                            self.systems.add_edge(layer[i], layer[j], ());
                            try_again = true;
                        }
                    }

                    for resource_j in &access_j.resources_written {
                        if access_i.resources_read.contains(resource_j)
                            || access_i.resources_written.contains(resource_j)
                        {
                            self.systems.add_edge(layer[j], layer[i], ());
                            try_again = true;
                        }
                    }

                    for component_i in &access_i.components_written {
                        if access_j.components_read.contains(component_i)
                            || access_j.components_written.contains(component_i)
                        {
                            self.systems.add_edge(layer[i], layer[j], ());
                            try_again = true;
                        }
                    }

                    for component_j in &access_j.components_written {
                        if access_i.components_read.contains(component_j)
                            || access_i.components_written.contains(component_j)
                        {
                            self.systems.add_edge(layer[j], layer[i], ());
                            try_again = true;
                        }
                    }
                }
            }
        }

        if try_again {
            self.resolve_dependencies(depth - 1)?;
        }

        Ok(())
    }

    pub fn run_concurrent(&mut self, world: Arc<World>) -> anyhow::Result<()> {
        self.resolve_dependencies(100)?;

        let layers = self.get_layers();

        // run each layer concurrently
        for layer in layers {
            let mut rxs = Vec::new();

            for node in layer {
                let (tx, rx) = crossbeam_channel::unbounded();
                rxs.push(rx);
                let world = world.clone();
                let system = self.systems[node].clone();
                rayon::spawn(move || {
                    let result = system.run(world);
                    tx.send(result).unwrap();
                });
            }

            for rx in rxs {
                rx.recv()??;
            }
        }

        Ok(())
    }
}
