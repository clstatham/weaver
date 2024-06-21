use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
};

use crate::{
    component::{Res, ResMut},
    prelude::{Query, ReadWorld, Resource, World, WorldLock, WriteWorld},
    query::{QueryAccess, QueryFetch, QueryFilter},
};
use petgraph::{prelude::*, visit::Topo};
use weaver_util::prelude::{anyhow, Result};

#[derive(Default, Clone)]
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
    fn run(&mut self, world: &WorldLock) -> Result<()>;
}

// NOTE: Not marking this as unsafe, unlike Bevy, since we don't actually violate memory safety in our implementation
// (Weaver lacks any form of `UnsafeWorldCell`)
pub trait ReadOnlySystem: System {}

pub trait IntoSystem<Marker>: Sized {
    type System: System;

    fn into_system(self) -> Self::System;
}

pub struct SystemState<P: SystemParam + 'static> {
    access: SystemAccess,
    state: P::State,
}

impl<P: SystemParam> SystemState<P> {
    pub fn access(&self) -> &SystemAccess {
        &self.access
    }

    pub fn get<'w, 's>(&'s mut self, world: &WorldLock) -> SystemParamFetch<'w, 's, P> {
        P::fetch(&mut self.state, world)
    }
}

pub trait SystemParam: Sized {
    type State: Send + Sync + 'static;
    type Fetch<'w, 's>: SystemParam<State = Self::State>;

    fn access() -> SystemAccess;

    fn init_state(world: &WorldLock) -> Self::State;

    #[allow(unused)]
    fn apply_deferred_mutations(state: &mut Self::State, world: &mut World) {}

    fn fetch<'w, 's>(state: &'s mut Self::State, world: &WorldLock) -> Self::Fetch<'w, 's>;
}

// NOTE: Not marking this as unsafe, unlike Bevy, since we don't actually violate memory safety in our implementation
// (Weaver lacks any form of `UnsafeWorldCell`)
pub trait ReadOnlySystemParam: SystemParam {}

pub type SystemParamFetch<'w, 's, P> = <P as SystemParam>::Fetch<'w, 's>;

pub struct ParamSet<'w, 's, T: SystemParam> {
    pub state: &'s mut T::State,
    _phantom: std::marker::PhantomData<&'w ()>,
}

impl<'w, 's, T: SystemParam> ParamSet<'w, 's, T> {
    pub fn new(state: &'s mut T::State) -> Self {
        Self {
            state,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'w2, 's2, T: SystemParam> SystemParam for ParamSet<'w2, 's2, T> {
    type State = T::State;
    type Fetch<'w, 's> = T::Fetch<'w, 's>;

    fn access() -> SystemAccess {
        T::access()
    }

    fn init_state(world: &WorldLock) -> Self::State {
        T::init_state(world)
    }

    fn fetch<'w, 's>(state: &'s mut Self::State, world: &WorldLock) -> Self::Fetch<'w, 's> {
        T::fetch(state, world)
    }
}

impl SystemParam for () {
    type State = ();
    type Fetch<'w, 's> = ();

    fn init_state(_: &WorldLock) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: Vec::new(),
            resources_written: Vec::new(),
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    fn fetch<'w, 's>(_: &'s mut Self::State, _world: &WorldLock) -> Self::Fetch<'w, 's> {}
}

impl ReadOnlySystemParam for () {}

impl<Q, F> SystemParam for Query<Q, F>
where
    Q: QueryFetch,
    F: QueryFilter,
{
    type State = ();
    type Fetch<'w, 's> = Query<Q, F>;

    fn init_state(_: &WorldLock) -> Self::State {}

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

    fn fetch<'w, 's>(_state: &'s mut Self::State, world: &WorldLock) -> Self::Fetch<'w, 's> {
        world.query_filtered()
    }
}

impl<Q, F> ReadOnlySystemParam for Query<Q, F>
where
    Q: QueryFetch,
    F: QueryFilter,
{
}

impl<T: Resource> SystemParam for Res<T> {
    type State = ();
    type Fetch<'w, 's> = Res<T>;

    fn init_state(_: &WorldLock) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: vec![TypeId::of::<T>()],
            resources_written: Vec::new(),
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    fn fetch<'w, 's>(_: &'s mut Self::State, world: &WorldLock) -> Self::Fetch<'w, 's> {
        world.get_resource::<T>().unwrap()
    }
}

impl<T: Resource> ReadOnlySystemParam for Res<T> {}

impl<T: Resource> SystemParam for ResMut<T> {
    type State = ();
    type Fetch<'w, 's> = ResMut<T>;

    fn init_state(_: &WorldLock) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: Vec::new(),
            resources_written: vec![TypeId::of::<T>()],
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    fn fetch<'w, 's>(_: &'s mut Self::State, world: &WorldLock) -> Self::Fetch<'w, 's> {
        world.get_resource_mut::<T>().unwrap()
    }
}

impl<T: Resource> ReadOnlySystemParam for ResMut<T> {}

impl SystemParam for WorldLock {
    type State = ();
    type Fetch<'w, 's> = WorldLock;

    fn init_state(_: &WorldLock) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: Vec::new(),
            resources_written: Vec::new(),
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    fn fetch<'w, 's>(_: &'s mut Self::State, world: &WorldLock) -> Self::Fetch<'w, 's> {
        world.clone()
    }
}

impl SystemParam for ReadWorld {
    type State = ();
    type Fetch<'w, 's> = Self;

    fn init_state(_: &WorldLock) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: Vec::new(),
            resources_written: Vec::new(),
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    fn fetch<'w, 's>(_: &'s mut Self::State, world: &WorldLock) -> Self::Fetch<'w, 's> {
        world.read()
    }
}

impl ReadOnlySystemParam for ReadWorld {}

impl SystemParam for WriteWorld {
    type State = ();
    type Fetch<'w, 's> = Self;

    fn init_state(_: &WorldLock) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: true,
            resources_read: Vec::new(),
            resources_written: Vec::new(),
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    fn fetch<'w, 's>(_: &'s mut Self::State, world: &WorldLock) -> Self::Fetch<'w, 's> {
        world.write()
    }
}

macro_rules! impl_system_param_tuple {
    ($($param:ident),*) => {
        #[allow(unused, non_snake_case)]
        impl<$($param),*> SystemParam for ($($param),*)
        where
            $($param: SystemParam),*
        {
            type State = ($($param::State),*);
            type Fetch<'w, 's> = ($($param::Fetch<'w, 's>),*);

            fn init_state(world: &WorldLock) -> Self::State {
                ($($param::init_state(world)),*)
            }

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

            fn fetch<'w, 's>(state: &'s mut Self::State, world: &WorldLock) -> Self::Fetch<'w, 's> {
                let ($($param),*) = state;
                ($($param::fetch($param, world)),*)
            }
        }

        #[allow(unused)]
        impl<$($param),*> ReadOnlySystemParam for ($($param),*)
        where
            $($param: ReadOnlySystemParam),*
        {
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

pub trait SystemParamFunction<M>: Send + Sync + 'static {
    type Param: SystemParam;

    fn run(&mut self, param: SystemParamFetch<Self::Param>) -> Result<()>;
}

pub struct FunctionSystem<M, F>
where
    F: SystemParamFunction<M>,
{
    access: SystemAccess,
    func: F,
    _marker: std::marker::PhantomData<fn() -> M>,
}

impl<M, F> IntoSystem<M> for F
where
    M: 'static,
    F: SystemParamFunction<M>,
{
    type System = FunctionSystem<M, F>;

    fn into_system(self) -> Self::System {
        let access = F::Param::access();
        FunctionSystem {
            access,
            func: self,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<M, F> System for FunctionSystem<M, F>
where
    M: 'static,
    F: SystemParamFunction<M>,
{
    fn access(&self) -> SystemAccess {
        self.access.clone()
    }

    fn run(&mut self, world: &WorldLock) -> Result<()> {
        let mut param = F::Param::init_state(world);
        let fetch = F::Param::fetch(&mut param, world);
        self.func.run(fetch)
    }
}

impl<M, F> ReadOnlySystem for FunctionSystem<M, F>
where
    M: 'static,
    F: SystemParamFunction<M>,
    <F as SystemParamFunction<M>>::Param: ReadOnlySystemParam,
{
}

macro_rules! impl_function_system {
    ($($param:ident),*) => {
        #[allow(unused, non_snake_case)]
        impl<Func, $($param,)*> SystemParamFunction<fn($($param,)*)> for Func
        where for<'a> &'a mut Func:
            FnMut($($param),*) -> Result<()>
            + FnMut($(SystemParamFetch<$param>),*) -> Result<()>,
            $($param: SystemParam),*,
            Func: Send + Sync + 'static,
        {
            type Param = ($($param),*);

            fn run(&mut self, param: SystemParamFetch<Self::Param>) -> Result<()> {
                fn inner<$($param,)*>(
                    mut func: impl FnMut($($param),*) -> Result<()>,
                    param: ($($param),*),
                ) -> Result<()> {
                    let ($($param),*) = param;
                    func($($param),*)
                }

                let ($($param),*) = param;
                inner(self, ($($param),*))
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

#[derive(Default)]
pub struct SystemGraph {
    systems: StableDiGraph<Box<dyn System>, ()>,
    index_cache: HashMap<TypeId, NodeIndex>,
}

impl SystemGraph {
    pub fn add_system<M, S>(&mut self, system: S) -> NodeIndex
    where
        S: IntoSystem<M> + 'static,
    {
        let node = self.systems.add_node(Box::new(system.into_system()));
        self.index_cache.insert(TypeId::of::<S>(), node);
        self.resolve_dependencies(100).unwrap();
        node
    }

    pub fn add_edge<M1, M2, S1, S2>(&mut self, _parent: S1, _child: S2)
    where
        S1: IntoSystem<M1> + 'static,
        S2: IntoSystem<M2> + 'static,
    {
        let parent = self.index_cache[&TypeId::of::<S1>()];
        let child = self.index_cache[&TypeId::of::<S2>()];
        self.systems.add_edge(parent, child, ());
        self.resolve_dependencies(100).unwrap();
    }

    pub fn add_system_after<M1, M2, S1, S2>(&mut self, system: S1, _after: S2)
    where
        S1: IntoSystem<M1> + 'static,
        S2: IntoSystem<M2> + 'static,
    {
        let node = self.add_system(system);
        let parent = self.index_cache[&TypeId::of::<S2>()];
        self.systems.add_edge(parent, node, ());
        self.resolve_dependencies(100).unwrap();
    }

    pub fn add_system_before<M1, M2, S1, S2>(&mut self, system: S1, _before: S2)
    where
        S1: IntoSystem<M1> + 'static,
        S2: IntoSystem<M2> + 'static,
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

    pub fn run(&mut self, world: &WorldLock) -> Result<()> {
        let mut schedule = Topo::new(&self.systems);
        while let Some(node) = schedule.next(&self.systems) {
            let system = &mut self.systems[node];
            system.run(world)?;
        }
        Ok(())
    }
}
