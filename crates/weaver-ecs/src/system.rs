use std::{any::TypeId, collections::VecDeque, future::Future, sync::Arc};

use crate::{
    component::{Component, Res, ResMut},
    prelude::World,
    world::ConstructFromWorld,
};
use futures::{future::BoxFuture, FutureExt};
use petgraph::prelude::*;
use weaver_util::{prelude::*, span};

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

    pub fn intersection(&self, other: &Self) -> Self {
        Self {
            resources_read: self
                .resources_read
                .intersection(&other.resources_read)
                .copied()
                .collect(),
            resources_written: self
                .resources_written
                .intersection(&other.resources_written)
                .copied()
                .collect(),
            components_read: self
                .components_read
                .intersection(&other.components_read)
                .copied()
                .collect(),
            components_written: self
                .components_written
                .intersection(&other.components_written)
                .copied()
                .collect(),
            exclusive: self.exclusive || other.exclusive,
        }
    }

    /// Returns true if the access is compatible with another access descriptor.
    /// Two accesses are compatible if they do not mutably access the same resource or component.
    pub fn is_compatible(&self, other: &Self) -> bool {
        let inter = self.intersection(other);
        inter.components_written.is_empty() && inter.resources_written.is_empty()
    }
}

/// A unit of work that can be executed on a world's resources and components.
pub trait System: Send + Sync {
    type Input;
    type Output;

    /// Returns the name of the system.
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }

    /// Returns the system access descriptor, describing what resources and components the system requires access to.
    fn access(&self) -> &SystemAccess;

    /// Initializes the system state.
    #[allow(unused)]
    fn initialize(&mut self, world: &mut World) {}

    /// Runs the system on the world.
    fn run(&mut self, world: &World) -> BoxFuture<'static, ()>;

    /// Returns true if the system can run on the world in its current state.
    #[allow(unused)]
    fn can_run(&self, world: &World) -> bool {
        true
    }
}

/// A type that can be converted into a system.
pub trait IntoSystem<Marker>: 'static + Send + Sync {
    type System: System<Input = (), Output = ()>;

    /// Converts the type into a boxed system.
    fn into_system(self) -> Box<Self::System>;
}

/// A parameter that can be used by a system to access resources and components in a world.
pub trait SystemParam: Send + Sync {
    type Item: SystemParam;
    type State: Send + Sync;

    fn access() -> SystemAccess;

    fn init_state(world: &World) -> Self::State;

    #[allow(unused)]
    fn update_state(state: &mut Self::State, world: &World) {}

    fn fetch(world: &World, state: &Self::State) -> Self::Item;

    #[allow(unused)]
    fn can_run(world: &World) -> bool {
        true
    }
}

pub type SystemParamItem<P> = <P as SystemParam>::Item;
pub type SystemParamState<P> = <P as SystemParam>::State;

impl SystemParam for () {
    type Item = ();
    type State = ();

    fn access() -> SystemAccess {
        SystemAccess::default()
    }

    fn init_state(_: &World) -> Self::State {}

    fn fetch(_: &World, _: &Self::State) -> Self::Item {}

    fn can_run(_: &World) -> bool {
        true
    }
}

pub struct Local<T: Send + Sync + ConstructFromWorld> {
    value: SharedLock<T>,
}

impl<T: Send + Sync + ConstructFromWorld> Clone for Local<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
        }
    }
}

impl<T: Send + Sync + ConstructFromWorld> Local<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: SharedLock::new(value),
        }
    }

    pub fn get(&self) -> Read<T> {
        self.value.read()
    }

    pub fn get_mut(&mut self) -> Write<T> {
        self.value.write()
    }
}

impl<T: Send + Sync + ConstructFromWorld> SystemParam for Local<T> {
    type Item = Local<T>;
    type State = Local<T>;

    fn access() -> SystemAccess {
        SystemAccess::default()
    }

    fn init_state(world: &World) -> Self::State {
        Local::new(T::from_world(world))
    }

    fn fetch(_world: &World, state: &Self::State) -> Self::Item {
        state.clone()
    }
}

impl<T: Component> SystemParam for Res<T> {
    type Item = Res<T>;
    type State = ();

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _: &Self::State) -> Self::Item {
        world.get_resource::<T>().unwrap()
    }

    fn can_run(world: &World) -> bool {
        if !world.has_resource::<T>() {
            log::debug!("Res: Resource {:?} is not available", T::type_name());
            return false;
        }
        true
    }
}

impl<T: Component> SystemParam for Option<Res<T>> {
    type Item = Option<Res<T>>;
    type State = ();

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _: &Self::State) -> Self::Item {
        world.get_resource::<T>()
    }

    fn can_run(_world: &World) -> bool {
        true
    }
}

impl<T: Component> SystemParam for ResMut<T> {
    type Item = ResMut<T>;
    type State = ();

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_written: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _: &Self::State) -> Self::Item {
        world.get_resource_mut::<T>().unwrap()
    }
    fn can_run(world: &World) -> bool {
        if !world.has_resource::<T>() {
            log::debug!("ResMut: Resource {:?} is not available", T::type_name());
            return false;
        }
        true
    }
}

impl<T: Component> SystemParam for Option<ResMut<T>> {
    type Item = Option<ResMut<T>>;
    type State = ();

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_written: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _: &Self::State) -> Self::Item {
        world.get_resource_mut::<T>()
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
            type State = ($( $param::State, )*);

            fn access() -> SystemAccess {
                let mut access = SystemAccess::default();
                $(
                    let param_access = $param::access();
                    if !access.is_compatible(&param_access) {
                        panic!("Incompatible access for system parameter {:?}", std::any::type_name::<$param>());
                    }
                    access.extend(param_access);
                )*
                access
            }

            fn init_state(world: &World) -> Self::State {
                ($( $param::init_state(world), )*)
            }

            fn fetch(world: &World, state: &Self::State) -> Self::Item {
                let ($( $param, )*) = state;
                ($( $param::fetch(world, $param), )*)
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

impl_system_param_tuple!(A);
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
    type State = <S as SystemParam>::State;

    fn access() -> SystemAccess {
        S::access()
    }

    fn init_state(world: &World) -> Self::State {
        <S as SystemParam>::init_state(world)
    }

    fn fetch(world: &World, state: &Self::State) -> Self::Item {
        SystemParamWrapper {
            param: <S>::fetch(world, state),
        }
    }

    fn can_run(world: &World) -> bool {
        S::can_run(world)
    }
}

pub trait SystemParamFunction<M>: 'static + Send + Sync {
    type Param: SystemParam + 'static;

    fn init_state(world: &World) -> SystemParamState<Self::Param>;

    fn update_state(state: &mut SystemParamState<Self::Param>, world: &World);

    fn run(&self, param: SystemParamItem<Self::Param>) -> impl Future<Output = ()> + Send + Sync;
}

pub struct FunctionSystem<M, F>
where
    M: 'static,
    F: SystemParamFunction<M>,
{
    func: Arc<F>,
    state: Option<SystemParamState<F::Param>>,
    access: SystemAccess,
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
            state: None,
            access: F::Param::access(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<M, F> System for FunctionSystem<M, F>
where
    M: 'static,
    F: SystemParamFunction<M>,
{
    type Input = ();
    type Output = ();

    fn name(&self) -> &str {
        std::any::type_name::<F>()
    }

    fn initialize(&mut self, world: &mut World) {
        self.state = Some(F::init_state(world));
    }

    fn access(&self) -> &SystemAccess {
        &self.access
    }

    fn run(&mut self, world: &World) -> BoxFuture<'static, ()> {
        let state = self.state.as_mut().expect("State not initialized");
        F::update_state(state, world);
        let fetch = F::Param::fetch(world, state);
        let func = self.func.clone();
        let span = span!(DEBUG, "FunctionSystem", name = self.name());
        async move {
            let _span = span.enter();
            func.run(fetch).await
        }
        .boxed()
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
            state: None,
            access: F::Param::access(),
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

            fn init_state(world: &World) -> SystemParamState<Self::Param> {
                ($($param::init_state(world)),*)
            }

            fn update_state(state: &mut SystemParamState<Self::Param>, world: &World) {
                let ($($param),*) = state;
                $($param::update_state($param, world);)*
            }

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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemAddOption {
    After(TypeId),
    Before(TypeId),
}

pub struct SystemConfig {
    system_type_id: TypeId,
    system: Box<dyn System<Input = (), Output = ()>>,
    options: FxHashSet<SystemAddOption>,
}

impl SystemConfig {
    pub fn new<M, S>(system: S) -> Self
    where
        M: 'static,
        S: IntoSystem<M>,
    {
        Self {
            system_type_id: TypeId::of::<S>(),
            system: system.into_system(),
            options: FxHashSet::default(),
        }
    }

    pub fn after<M2: 'static, T: IntoSystem<M2>>(mut self, _system: T) -> Self {
        if self
            .options
            .iter()
            .any(|o| o == &SystemAddOption::Before(TypeId::of::<T>()))
        {
            panic!("Cannot add system both after and before another system");
        }
        self.options
            .insert(SystemAddOption::After(TypeId::of::<T>()));
        self
    }

    pub fn before<M2: 'static, T: IntoSystem<M2>>(mut self, _system: T) -> Self {
        if self
            .options
            .iter()
            .any(|o| o == &SystemAddOption::After(TypeId::of::<T>()))
        {
            panic!("Cannot add system both after and before another system");
        }
        self.options
            .insert(SystemAddOption::Before(TypeId::of::<T>()));
        self
    }
}

pub trait IntoSystemConfig<M>: Sized + 'static {
    fn finish(self) -> SystemConfig;

    fn after<M2: 'static, T: IntoSystem<M2>>(self, system: T) -> SystemConfig {
        self.finish().after(system)
    }

    fn before<M2: 'static, T: IntoSystem<M2>>(self, system: T) -> SystemConfig {
        self.finish().before(system)
    }
}

impl<M, I, S> IntoSystemConfig<M> for I
where
    M: 'static,
    I: IntoSystem<M, System = S>,
    S: System<Input = (), Output = ()>,
{
    fn finish(self) -> SystemConfig {
        SystemConfig::new(self)
    }
}

impl IntoSystemConfig<()> for SystemConfig {
    fn finish(self) -> SystemConfig {
        self
    }
}

#[derive(Default)]
pub struct SystemGraph {
    systems: StableDiGraph<SharedLock<Box<dyn System<Input = (), Output = ()>>>, ()>,
    index_cache: TypeIdMap<NodeIndex>,
}

impl SystemGraph {
    /// Adds a system to the graph.
    pub fn add_system<M, S>(&mut self, system: S) -> NodeIndex
    where
        M: 'static,
        S: IntoSystemConfig<M>,
    {
        let config = system.finish();
        let SystemConfig {
            system_type_id,
            system,
            options,
            ..
        } = config;

        let node = self.systems.add_node(SharedLock::new(system));
        self.index_cache.insert(system_type_id, node);

        for option in options {
            let index = self.index_cache[&system_type_id];
            match option {
                SystemAddOption::After(id) => {
                    let other = self.index_cache[&id];
                    self.systems.add_edge(other, index, ());
                }
                SystemAddOption::Before(id) => {
                    let other = self.index_cache[&id];
                    self.systems.add_edge(index, other, ());
                }
            }
        }

        node
    }

    pub fn add_edge<SM, TM, S, T>(&mut self, _from: S, _to: T)
    where
        SM: 'static,
        TM: 'static,
        S: IntoSystem<SM>,
        T: IntoSystem<TM>,
        S::System: System,
        T::System: System,
    {
        let from = self.index_cache[&TypeId::of::<S>()];
        let to = self.index_cache[&TypeId::of::<T>()];
        self.systems.add_edge(from, to, ());
    }

    /// Returns true if the graph contains the system.
    pub fn has_system<M: 'static, S: IntoSystem<M>>(&self, _system: &S) -> bool {
        self.index_cache.contains_key(&TypeId::of::<S>())
    }

    /// Sorts the graph based on system dependencies, returning a list of layers where each layer contains systems that can be run in parallel.
    /// This will respect existing system dependencies, and will not add any new ones.
    fn get_batches(&self) -> Vec<Vec<NodeIndex>> {
        let mut batches = Vec::new();
        let mut queue = VecDeque::new();

        // calculate the number of incoming edges (in-degrees) for each node
        let mut in_degrees = FxHashMap::default();
        for node in self.systems.node_indices() {
            let in_degree = self.systems.neighbors_directed(node, Incoming).count();
            in_degrees.insert(node, in_degree);

            if in_degree == 0 {
                queue.push_back(node);
            }
        }

        while !queue.is_empty() {
            let mut batch = Vec::new();

            for _ in 0..queue.len() {
                let node = queue.pop_front().unwrap();
                batch.push(node);

                for child in self.systems.neighbors_directed(node, Outgoing) {
                    let in_degree = in_degrees.get_mut(&child).unwrap();
                    *in_degree -= 1;
                    if *in_degree == 0 {
                        queue.push_back(child);
                    }
                }
            }

            batches.push(batch);
        }

        batches
    }

    /// Resolves system dependencies, ensuring that no system mutably accesses the same resource or component at the same time as another system.
    pub fn resolve_dependencies(&mut self, depth: usize) -> Result<()> {
        if depth == 0 {
            return Err(anyhow!(
                "Cyclic system dependency detected (depth limit reached)"
            ));
        }
        if petgraph::algo::is_cyclic_directed(&self.systems) {
            let sccs = petgraph::algo::kosaraju_scc(&self.systems);
            for scc in sccs {
                if scc.len() > 1 {
                    let mut names = Vec::new();
                    for node in scc {
                        let system = &self.systems[node];
                        names.push(system.read().name().to_string());
                    }
                    log::error!("Cyclic system dependency detected in: {:#?}", names);
                }
            }
            return Err(anyhow!(
                "Cyclic system dependency detected (depth = {})",
                depth
            ));
        }

        let layers = self.get_batches();

        // detect systems that access the same resource or component mutably
        // and add dependencies to ensure that they run in sequence
        for layer in &layers {
            for i in 0..layer.len() {
                let node = layer[i];
                let system = &self.systems[node];
                let access = system.read().access().clone();
                #[allow(clippy::needless_range_loop)]
                for j in i + 1..layer.len() {
                    let other = layer[j];
                    let other_system = &self.systems[other];
                    let other_access = other_system.read().access().clone();
                    if !access.is_compatible(&other_access) {
                        if self.systems.contains_edge(other, node)
                            || self.systems.contains_edge(node, other)
                        {
                            continue;
                        }
                        self.systems.add_edge(node, other, ());
                        return self.resolve_dependencies(depth - 1);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn initialize(&mut self, world: &mut World) {
        self.resolve_dependencies(100).unwrap();

        for node in self.systems.node_indices() {
            let system = &self.systems[node];
            system.write().initialize(world);
        }
    }

    /// Runs all systems in the graph.
    pub async fn run(&mut self, world: &mut World) -> Result<()> {
        let schedule = self.get_batches();
        for layer in schedule {
            let mut names = Vec::new();
            for node in &layer {
                let system = &self.systems[*node];
                names.push(system.read().name().to_string());
            }
            let span = span!(DEBUG, "System Batch", names = format!("{:?}", names));
            let _span = span.enter();
            // log::debug!("Running systems: {:?}", names);
            let mut handles = Vec::new();
            for node in layer {
                let system = &self.systems[node];
                if !system.read().can_run(world) {
                    log::debug!("Skipping system: {}", system.read().name());
                    continue;
                }
                let handle = tokio::spawn(system.write().run(world));
                handles.push(handle);
            }

            loop {
                if handles.iter().all(|handle| handle.is_finished()) {
                    break;
                }

                world.apply_commands();
                tokio::task::yield_now().await;
            }
        }

        Ok(())
    }
}
