use std::{
    any::{Any, TypeId},
    future::Future,
    sync::Arc,
};

use crate::{
    component::{Res, ResMut},
    prelude::World,
};
use futures::{future::BoxFuture, FutureExt};
use petgraph::{prelude::*, visit::Topo};
use weaver_util::{
    lock::SharedLock,
    {anyhow, FxHashMap, FxHashSet, Result},
};

/// A system access descriptor, indicating what resources and components a system reads and writes. This is used to validate system access at runtime.
#[derive(Default, Clone)]
pub struct SystemAccess {
    pub resources_read: FxHashSet<TypeId>,
    pub resources_written: FxHashSet<TypeId>,
    pub components_read: FxHashSet<TypeId>,
    pub components_written: FxHashSet<TypeId>,
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

    /// Returns true if the access is compatible with another access descriptor.
    /// Two accesses are compatible if they do not mutably access the same resource or component.
    pub fn is_compatible(&self, other: &Self) -> bool {
        for resource in &other.resources_written {
            if self.resources_read.contains(resource) {
                return false;
            }
            if self.resources_written.contains(resource) {
                return false;
            }
        }

        for resource in &self.resources_written {
            if other.resources_read.contains(resource) {
                return false;
            }
            if other.resources_written.contains(resource) {
                return false;
            }
        }

        for component in &other.components_written {
            if self.components_read.contains(component) {
                return false;
            }
            if self.components_written.contains(component) {
                return false;
            }
        }

        for component in &self.components_written {
            if other.components_read.contains(component) {
                return false;
            }
            if other.components_written.contains(component) {
                return false;
            }
        }

        true
    }
}

/// A single unit of work that can be executed on a world.
pub trait System: Send + Sync {
    /// Returns the name of the system.
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }

    /// Returns the system access descriptor, describing what resources and components the system requires access to.
    fn access(&self) -> SystemAccess;
    #[allow(unused)]

    /// Initializes the system state.
    fn initialize(&mut self, world: &mut World) {}

    /// Runs the system on the world.
    fn run(&self, world: &World) -> BoxFuture<'static, ()>;

    /// Returns true if the system can run on the world in its current state.
    #[allow(unused)]
    fn can_run(&self, world: &World) -> bool {
        true
    }
}

impl System for Box<dyn System> {
    fn name(&self) -> &str {
        self.as_ref().name()
    }

    fn access(&self) -> SystemAccess {
        self.as_ref().access()
    }

    fn initialize(&mut self, world: &mut World) {
        self.as_mut().initialize(world)
    }

    fn run(&self, world: &World) -> BoxFuture<'static, ()> {
        self.as_ref().run(world)
    }

    fn can_run(&self, world: &World) -> bool {
        self.as_ref().can_run(world)
    }
}

/// A type that can be converted into a system.
pub trait IntoSystem<Marker>: 'static + Send + Sync {
    type System: System;

    /// Converts the type into a boxed system.
    fn into_system(self) -> Box<Self::System>;
}

impl IntoSystem<Box<dyn System>> for Box<dyn System> {
    type System = Box<dyn System>;

    fn into_system(self) -> Box<Self::System> {
        Box::new(self)
    }
}

/// # Safety
///
/// Caller must ensure that all system params being used are valid for simultaneous access.
pub trait SystemParam: Send + Sync {
    type Item: SystemParam;

    fn validate_access(access: &SystemAccess) -> bool {
        Self::access().is_compatible(access)
    }

    fn access() -> SystemAccess;

    fn fetch(world: &World) -> Self::Item;

    #[allow(unused)]
    fn can_run(world: &World) -> bool {
        true
    }
}

pub type SystemParamItem<P> = <P as SystemParam>::Item;

impl SystemParam for () {
    type Item = ();

    fn access() -> SystemAccess {
        SystemAccess::default()
    }

    fn fetch(_: &World) -> Self::Item {}

    fn can_run(_: &World) -> bool {
        true
    }
}

impl<T: Any + Send + Sync> SystemParam for Res<T> {
    type Item = Res<T>;

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    fn fetch(world: &World) -> Self::Item {
        world.get_resource::<T>().unwrap()
    }

    fn validate_access(access: &SystemAccess) -> bool {
        !access.resources_written.contains(&TypeId::of::<T>())
    }

    fn can_run(world: &World) -> bool {
        if !world.has_resource::<T>() {
            log::debug!(
                "Res: Resource {:?} is not available",
                std::any::type_name::<T>()
            );
            return false;
        }
        true
    }
}

impl<T: Any + Send + Sync> SystemParam for Option<Res<T>> {
    type Item = Option<Res<T>>;

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    fn fetch(world: &World) -> Self::Item {
        world.get_resource::<T>()
    }

    fn validate_access(access: &SystemAccess) -> bool {
        !access.resources_written.contains(&TypeId::of::<T>())
    }

    fn can_run(_world: &World) -> bool {
        true
    }
}

impl<T: Any + Send + Sync> SystemParam for ResMut<T> {
    type Item = ResMut<T>;

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_written: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    fn fetch(world: &World) -> Self::Item {
        world.get_resource_mut::<T>().unwrap()
    }

    fn validate_access(access: &SystemAccess) -> bool {
        !access.resources_read.contains(&TypeId::of::<T>())
            && !access.resources_written.contains(&TypeId::of::<T>())
    }

    fn can_run(world: &World) -> bool {
        if !world.has_resource::<T>() {
            log::debug!(
                "ResMut: Resource {:?} is not available",
                std::any::type_name::<T>()
            );
            return false;
        }
        true
    }
}

impl<T: Any + Send + Sync> SystemParam for Option<ResMut<T>> {
    type Item = Option<ResMut<T>>;

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_written: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    fn fetch(world: &World) -> Self::Item {
        world.get_resource_mut::<T>()
    }

    fn validate_access(access: &SystemAccess) -> bool {
        !access.resources_read.contains(&TypeId::of::<T>())
            && !access.resources_written.contains(&TypeId::of::<T>())
    }

    fn can_run(_world: &World) -> bool {
        true
    }
}

macro_rules! impl_system_param_tuple {
    ($( $param:ident ),*) => {
        impl<$( $param: SystemParam ),*> SystemParam for ($( $param, )*)
        {
            type Item = ($( $param::Item, )*);

            fn access() -> SystemAccess {
                let mut access = SystemAccess::default();
                $(
                    access.extend($param::access());
                )*
                access
            }

            fn fetch(world: &World) -> Self::Item {
                ($( $param::fetch(world), )*)
            }

            fn can_run(world: &World) -> bool {
                $(
                    if !$param::can_run(world) {
                        return false;
                    }
                )*
                true
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
impl_system_param_tuple!(A, B, C, D, E, F, G, H, I);
impl_system_param_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_system_param_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_system_param_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_system_param_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_system_param_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_system_param_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_system_param_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
impl_system_param_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q);
impl_system_param_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R);
impl_system_param_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S);
impl_system_param_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T);

pub struct SystemParamWrapper<S: SystemParam> {
    pub param: S::Item,
}

impl<S: SystemParam> SystemParamWrapper<S> {
    pub fn new(param: S::Item) -> Self {
        Self { param }
    }

    pub fn item(&self) -> &S::Item {
        &self.param
    }

    pub fn item_mut(&mut self) -> &mut S::Item {
        &mut self.param
    }
}

impl<S: SystemParam> SystemParam for SystemParamWrapper<S> {
    type Item = SystemParamWrapper<S>;

    fn access() -> SystemAccess {
        S::access()
    }

    fn fetch(world: &World) -> Self::Item {
        SystemParamWrapper {
            param: S::fetch(world),
        }
    }

    fn can_run(world: &World) -> bool {
        S::can_run(world)
    }
}

pub trait SystemParamFunction<M>: 'static + Send + Sync {
    type Param: SystemParam + 'static;

    fn run(&self, param: SystemParamItem<Self::Param>) -> impl Future<Output = ()> + Send + Sync;
}

pub struct FunctionSystem<M, F>
where
    M: 'static,
    F: SystemParamFunction<M>,
{
    func: Arc<F>,
    _marker: std::marker::PhantomData<fn() -> M>,
}

impl<M, F> FunctionSystem<M, F>
where
    M: 'static,
    F: SystemParamFunction<M>,
{
    pub fn new(func: F) -> Self {
        Self {
            func: Arc::new(func),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<M, F> System for FunctionSystem<M, F>
where
    M: 'static,
    F: SystemParamFunction<M>,
{
    fn name(&self) -> &str {
        std::any::type_name::<F>()
    }

    fn initialize(&mut self, _world: &mut World) {}

    fn access(&self) -> SystemAccess {
        F::Param::access()
    }

    fn run(&self, world: &World) -> BoxFuture<'static, ()> {
        let fetch = F::Param::fetch(world);
        let func = self.func.clone();
        async move { func.run(fetch).await }.boxed()
        // async move {
        //     let mut f = func.write().take().unwrap();
        //     f.run(fetch).await;
        //     *func.write() = Some(f);
        // }
        // .boxed()
    }

    fn can_run(&self, world: &World) -> bool {
        F::Param::can_run(world)
    }
}

pub struct FunctionSystemMarker;

impl<M, F> IntoSystem<(FunctionSystemMarker, M)> for F
where
    M: 'static,
    F: SystemParamFunction<M>,
{
    type System = FunctionSystem<M, F>;

    fn into_system(self) -> Box<Self::System> {
        Box::new(FunctionSystem {
            func: Arc::new(self),
            _marker: std::marker::PhantomData,
        })
    }
}

macro_rules! impl_function_system {
    ($($param:ident),*) => {
        #[allow(unused, non_snake_case)]
        impl<Func, Fut, $($param,)*> SystemParamFunction<fn($($param,)*)> for Func
        where for<'a> &'a Func:
            Fn($($param),*) -> Fut
            + Fn($(SystemParamItem<$param>),*) -> Fut,
            $($param: SystemParam + 'static),*,
            Func: 'static + Send + Sync,
            Fut: Future<Output = ()> + Send + Sync,
        {
            type Param = ($($param),*);

            async fn run(&self, param: SystemParamItem<Self::Param>) {
                async fn inner<Fut, $($param,)*>(
                    mut func: impl Fn($($param),*) -> Fut,
                    param: ($($param),*),
                )
                where
                    Fut: Future<Output = ()> + Send + Sync,
                {
                    let ($($param),*) = param;
                    func($($param),*).await;
                }

                let ($($param),*) = param;
                inner(self, ($($param),*)).await;
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
impl_function_system!(A, B, C, D, E, F, G, H, I);
impl_function_system!(A, B, C, D, E, F, G, H, I, J);
impl_function_system!(A, B, C, D, E, F, G, H, I, J, K);
impl_function_system!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_function_system!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_function_system!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_function_system!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_function_system!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
impl_function_system!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q);
impl_function_system!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R);
impl_function_system!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S);
impl_function_system!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T);

#[derive(Default)]
pub struct SystemGraph {
    systems: StableDiGraph<SharedLock<Box<dyn System>>, ()>,
    index_cache: FxHashMap<TypeId, NodeIndex>,
}

impl SystemGraph {
    /// Adds a system to the graph.
    pub fn add_system<M, S>(&mut self, system: S) -> NodeIndex
    where
        M: 'static,
        S: IntoSystem<M>,
        S::System: System,
    {
        let node = self.systems.add_node(SharedLock::new(system.into_system()));
        self.index_cache.insert(TypeId::of::<S>(), node);
        node
    }

    /// Adds a dependency between two systems in the graph.
    pub fn add_edge<M1, M2, BEFORE, AFTER>(&mut self, _before: BEFORE, _after: AFTER)
    where
        M1: 'static,
        M2: 'static,
        BEFORE: IntoSystem<M1>,
        AFTER: IntoSystem<M2>,
    {
        let parent = self.index_cache[&TypeId::of::<BEFORE>()];
        let child = self.index_cache[&TypeId::of::<AFTER>()];
        self.systems.add_edge(parent, child, ());
    }

    /// Adds a system to the graph that will always run after another system.
    pub fn add_system_after<M1, M2, S, AFTER>(&mut self, system: S, _after: AFTER)
    where
        M1: 'static,
        M2: 'static,
        S: IntoSystem<M1>,
        AFTER: IntoSystem<M2>,
        S::System: System,
        AFTER::System: System,
    {
        let node = self.add_system(system);
        let parent = self.index_cache[&TypeId::of::<AFTER>()];
        self.systems.add_edge(parent, node, ());
    }

    /// Adds a system to the graph that will always run before another system.
    pub fn add_system_before<M1, M2, S, BEFORE>(&mut self, system: S, _before: BEFORE)
    where
        M1: 'static,
        M2: 'static,
        S: IntoSystem<M1>,
        BEFORE: IntoSystem<M2>,
        S::System: System,
        BEFORE::System: System,
    {
        let node = self.add_system(system);
        let child = self.index_cache[&TypeId::of::<BEFORE>()];
        self.systems.add_edge(node, child, ());
    }

    /// Returns true if the graph contains the system.
    pub fn has_system<M: 'static, S: IntoSystem<M>>(&self, _system: &S) -> bool {
        self.index_cache.contains_key(&TypeId::of::<S>())
    }

    /// Sorts the graph based on system dependencies, returning a list of layers where each layer contains systems that can be run in parallel.
    /// This will respect existing system dependencies, and will not add any new ones.
    fn get_layers(&self) -> Vec<Vec<NodeIndex>> {
        let mut schedule = Topo::new(&self.systems);

        let mut seen = FxHashSet::default();
        let mut layers = Vec::new();
        let mut current_layer = Vec::new();
        while let Some(node) = schedule.next(&self.systems) {
            if seen.contains(&node) {
                continue;
            }
            seen.insert(node);
            current_layer.push(node);

            if self.systems[node].read().access().exclusive {
                layers.push(current_layer);
                current_layer = Vec::new();
                continue;
            }

            if self
                .systems
                .neighbors_directed(node, Direction::Incoming)
                .count()
                == 0
            {
                layers.push(current_layer);
                current_layer = Vec::new();
                continue;
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

    /// Resolves system dependencies, ensuring that no system mutably accesses the same resource or component at the same time as another system.
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
                    let access_i = system_i.read().access();
                    let access_j = system_j.read().access();

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

    /// Runs all systems in the graph.
    pub async fn run(&mut self, world: &mut World) -> Result<()> {
        let mut schedule = Topo::new(&self.systems);
        while let Some(node) = schedule.next(&self.systems) {
            let system = &self.systems[node];
            system.write().initialize(world);
            if !system.read().can_run(world) {
                log::debug!("Skipping system: {}", system.read().name());
                continue;
            }
            log::trace!("Running system: {}", system.read().name());

            let handle = tokio::spawn(system.write().run(world));
            while !handle.is_finished() {
                world.apply_commands();
                tokio::task::yield_now().await;
            }
        }
        // world.await_all_tasks().await;

        Ok(())
    }
}
