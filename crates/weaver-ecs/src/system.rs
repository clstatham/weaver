use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    component::{Res, ResMut},
    prelude::{Query, Resource, World},
    query::{QueryAccess, QueryFetch, QueryFilter},
};
use petgraph::{prelude::*, visit::Topo};
use weaver_util::prelude::{anyhow, Result};

#[derive(Default)]
pub struct SystemAccess {
    pub resources_read: Vec<TypeId>,
    pub resources_written: Vec<TypeId>,
    pub components_read: Vec<TypeId>,
    pub components_written: Vec<TypeId>,
    pub exclusive: bool,
}

impl SystemAccess {
    pub fn extend(&mut self, other: SystemAccess) {
        self.resources_read.extend(other.resources_read);
        self.resources_written.extend(other.resources_written);
        self.components_read.extend(other.components_read);
        self.components_written.extend(other.components_written);
        self.exclusive |= other.exclusive;
    }
}

pub trait System: 'static + Send + Sync {
    fn access(&self) -> SystemAccess;
    fn run(&self, world: &mut World) -> Result<()>;
}

pub trait SystemParam {
    fn access() -> SystemAccess;
    fn fetch(world: &World) -> Option<Self>
    where
        Self: Sized;
}

impl<T: SystemParam> SystemParam for Option<T> {
    fn access() -> SystemAccess {
        T::access()
    }

    fn fetch(world: &World) -> Option<Self>
    where
        Self: Sized,
    {
        T::fetch(world).map(Some)
    }
}

impl<Q, F> SystemParam for Query<Q, F>
where
    Q: QueryFetch,
    F: QueryFilter,
{
    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
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

    fn fetch(world: &World) -> Option<Self> {
        Some(Query::new(world))
    }
}

impl<T: Resource> SystemParam for Res<T> {
    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: vec![TypeId::of::<T>()],
            resources_written: Vec::new(),
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    fn fetch(world: &World) -> Option<Self> {
        world.get_resource::<T>()
    }
}

impl<T: Resource> SystemParam for ResMut<T> {
    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: Vec::new(),
            resources_written: vec![TypeId::of::<T>()],
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    fn fetch(world: &World) -> Option<Self> {
        world.get_resource_mut::<T>()
    }
}

macro_rules! impl_system_param_tuple {
    ($($param:ident),*) => {
        #[allow(unused)]
        impl<$($param),*> SystemParam for ($($param),*)
        where
            $($param: SystemParam),*
        {
            fn access() -> SystemAccess {
                let mut access = SystemAccess {
                    exclusive: false,
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

            fn fetch(world: &World) -> Option<Self> {
                Some(($($param::fetch(world)?),*))
            }
        }
    };
}

impl_system_param_tuple!(A, B);
impl_system_param_tuple!(A, B, C);
impl_system_param_tuple!(A, B, C, D);
impl_system_param_tuple!(A, B, C, D, E);
impl_system_param_tuple!(A, B, C, D, E, F);
impl_system_param_tuple!(A, B, C, D, E, F, G);
impl_system_param_tuple!(A, B, C, D, E, F, G, H);

pub trait FunctionSystem<Marker>: 'static + Send + Sync {
    fn into_system(self) -> Arc<dyn System>;
}

macro_rules! impl_function_system {
    ($($param:ident),*) => {
        impl<Func, $($param),*> FunctionSystem<fn($($param),*)> for Func
        where
            Func: Fn($($param),*) -> Result<()> + 'static + Send + Sync,
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
                    Func: Fn($($param),*) -> Result<()> + 'static + Send + Sync,
                    $($param: SystemParam + 'static + Send + Sync),*
                {
                    fn access(&self) -> SystemAccess {
                        let mut access = SystemAccess {
                            exclusive: false,
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

                    fn run(&self, world: &mut World) -> Result<()> {
                        let ($($param),*) = ($($param::fetch(world).unwrap()),*);
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
    Func: Fn(&mut World) -> Result<()> + 'static + Send + Sync,
{
    fn into_system(self) -> Arc<dyn System> {
        struct FunctionSystemImpl<Func> {
            func: Func,
        }

        impl<Func> System for FunctionSystemImpl<Func>
        where
            Func: Fn(&mut World) -> Result<()> + 'static + Send + Sync,
        {
            fn access(&self) -> SystemAccess {
                SystemAccess {
                    exclusive: true,
                    resources_read: Vec::new(),
                    resources_written: Vec::new(),
                    components_read: Vec::new(),
                    components_written: Vec::new(),
                }
            }
            fn run(&self, world: &mut World) -> Result<()> {
                (self.func)(world)
            }
        }

        Arc::new(FunctionSystemImpl { func: self })
    }
}

#[derive(Default)]
pub struct SystemGraph {
    systems: StableDiGraph<Arc<dyn System>, ()>,
    index_cache: HashMap<TypeId, NodeIndex>,
}

impl SystemGraph {
    pub fn add_system<M, S>(&mut self, system: S) -> NodeIndex
    where
        S: FunctionSystem<M>,
    {
        let node = self.systems.add_node(system.into_system());
        self.index_cache.insert(TypeId::of::<S>(), node);
        self.resolve_dependencies(100).unwrap();
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
        self.resolve_dependencies(100).unwrap();
    }

    pub fn add_system_after<M1, M2, S1, S2>(&mut self, system: S1, _after: S2)
    where
        S1: FunctionSystem<M1>,
        S2: FunctionSystem<M2>,
    {
        let node = self.add_system(system);
        let parent = self.index_cache[&TypeId::of::<S2>()];
        self.systems.add_edge(parent, node, ());
        self.resolve_dependencies(100).unwrap();
    }

    pub fn add_system_before<M1, M2, S1, S2>(&mut self, system: S1, _before: S2)
    where
        S1: FunctionSystem<M1>,
        S2: FunctionSystem<M2>,
    {
        let node = self.add_system(system);
        let child = self.index_cache[&TypeId::of::<S2>()];
        self.systems.add_edge(node, child, ());
        self.resolve_dependencies(100).unwrap();
    }

    pub fn get_layers(&self) -> Vec<Vec<NodeIndex>> {
        let mut schedule = Topo::new(&self.systems);

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

    pub fn resolve_dependencies(&mut self, depth: usize) -> Result<()> {
        if depth == 0 {
            return Err(anyhow!("Cyclic system dependency detected"));
        }
        let layers = self.get_layers();

        let mut try_again = false;

        // only one system at a time can access a resource or component mutably
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

    pub fn run(&self, world: &mut World) -> Result<()> {
        let mut schedule = Topo::new(&self.systems);
        while let Some(node) = schedule.next(&self.systems) {
            let system = self.systems[node].clone();
            system.run(world)?;
        }
        Ok(())
    }
}
