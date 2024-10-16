use std::any::TypeId;

use crate::{
    component::{Res, ResMut},
    prelude::{Resource, UnsafeWorldCell, World},
};
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
    type Output;

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
    fn run(&mut self, world: &mut World) -> Self::Output;

    /// Returns true if the system can run on the world in its current state.
    #[allow(unused)]
    fn can_run(&self, world: &World) -> bool {
        true
    }
}

/// A type that can be converted into a system.
pub trait IntoSystem<Marker>: 'static {
    type System: System;

    /// Returns the name of the system.
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }

    /// Converts the type into a boxed system.
    fn into_system(self) -> Box<Self::System>;
}

pub struct SystemState<P: SystemParam + 'static> {
    access: SystemAccess,
    state: P::State,
}

impl<P: SystemParam> SystemState<P> {
    pub fn new(world: &mut World) -> Self {
        Self {
            access: P::access(),
            state: P::init_state(world),
        }
    }

    pub fn access(&self) -> &SystemAccess {
        &self.access
    }

    pub fn get<'w, 's>(&'s mut self, world: UnsafeWorldCell<'w>) -> SystemParamItem<'w, 's, P> {
        // validate access
        self.access();
        unsafe { P::fetch(&mut self.state, world) }
    }

    pub fn can_run(&self, world: &World) -> bool {
        P::can_run(world)
    }
}

unsafe impl<P: SystemParam> SystemParam for SystemState<P> {
    type State = P::State;
    type Item<'w, 's> = P::Item<'w, 's>;

    fn access() -> SystemAccess {
        P::access()
    }

    fn init_state(world: &mut World) -> Self::State {
        P::init_state(world)
    }

    unsafe fn fetch<'w, 's>(
        state: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        // validate access
        P::access();
        unsafe { P::fetch(state, world) }
    }

    fn can_run(world: &World) -> bool {
        P::can_run(world)
    }
}

pub struct SystemParamWrapper<'w, 's, P: SystemParam> {
    item: SystemParamItem<'w, 's, P>,
}

impl<'w, 's, P: SystemParam> SystemParamWrapper<'w, 's, P> {
    #[inline]
    pub fn item(&self) -> &P::Item<'w, 's> {
        &self.item
    }

    #[inline]
    pub fn item_mut(&mut self) -> &mut P::Item<'w, 's> {
        &mut self.item
    }

    #[inline]
    pub fn into_inner(self) -> P::Item<'w, 's> {
        self.item
    }
}

unsafe impl<'w2, 's2, P: SystemParam> SystemParam for SystemParamWrapper<'w2, 's2, P> {
    type State = P::State;
    type Item<'w, 's> = SystemParamWrapper<'w, 's, P>;

    fn access() -> SystemAccess {
        P::access()
    }

    fn init_state(world: &mut World) -> Self::State {
        P::init_state(world)
    }

    unsafe fn fetch<'w, 's>(
        state: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        unsafe {
            SystemParamWrapper {
                item: P::fetch(state, world),
            }
        }
    }

    fn can_run(world: &World) -> bool {
        P::can_run(world)
    }
}

/// # Safety
///
/// Caller must ensure that all system params being used are valid for simultaneous access.
pub unsafe trait SystemParam {
    type State: Sized + Send + Sync;
    type Item<'w, 's>: SystemParam<State = Self::State>;

    fn validate_access(access: &SystemAccess) -> bool {
        Self::access().is_compatible(access)
    }

    fn access() -> SystemAccess;

    fn init_state(world: &mut World) -> Self::State;

    /// # Safety
    ///
    /// Caller must ensure that all system params being used are valid for simultaneous access.
    unsafe fn fetch<'w, 's>(
        state: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's>;

    #[allow(unused)]
    fn apply(state: &mut Self::State, world: &mut World) {}

    #[allow(unused)]
    fn can_run(world: &World) -> bool {
        true
    }
}

pub type SystemParamItem<'w, 's, P> = <P as SystemParam>::Item<'w, 's>;

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

unsafe impl<T: SystemParam> SystemParam for ParamSet<'_, '_, T> {
    type State = T::State;
    type Item<'w, 's> = T::Item<'w, 's>;

    fn access() -> SystemAccess {
        T::access()
    }

    fn init_state(world: &mut World) -> Self::State {
        T::init_state(world)
    }

    unsafe fn fetch<'w, 's>(
        state: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        unsafe { T::fetch(state, world) }
    }

    fn can_run(world: &World) -> bool {
        T::can_run(world)
    }
}

unsafe impl SystemParam for () {
    type State = ();
    type Item<'w, 's> = ();

    fn init_state(_: &mut World) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            ..Default::default()
        }
    }

    unsafe fn fetch<'w, 's>(
        _: &'s mut Self::State,
        _world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
    }

    fn can_run(_: &World) -> bool {
        true
    }
}

unsafe impl<T: Resource> SystemParam for Res<'_, T> {
    type State = ();
    type Item<'w, 's> = Res<'w, T>;

    fn init_state(_: &mut World) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    /// # Safety
    ///
    /// Caller must ensure that the resource exists and that we have shared access to it
    unsafe fn fetch<'w, 's>(
        _: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        unsafe { world.get_resource::<T>().unwrap() }
    }

    fn can_run(world: &World) -> bool {
        if !world.has_resource::<T>() {
            log::debug!("Res: Missing resource: {}", std::any::type_name::<T>());
            return false;
        }
        true
    }
}

unsafe impl<T: Resource> SystemParam for Option<Res<'_, T>> {
    type State = ();
    type Item<'w, 's> = Option<Res<'w, T>>;

    fn init_state(_: &mut World) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    /// # Safety
    ///
    /// Caller must ensure that the resource exists and that we have shared access to it
    unsafe fn fetch<'w, 's>(
        _: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        unsafe { world.get_resource::<T>() }
    }

    fn can_run(_world: &World) -> bool {
        true
    }
}

unsafe impl<T: Resource> SystemParam for ResMut<'_, T> {
    type State = ();
    type Item<'w, 's> = ResMut<'w, T>;

    fn init_state(_: &mut World) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_written: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    /// # Safety
    ///
    /// Caller must ensure that the resource exists and that we have exclusive access to it
    unsafe fn fetch<'w, 's>(
        _: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        unsafe { world.get_resource_mut::<T>().unwrap() }
    }

    fn can_run(world: &World) -> bool {
        if !world.has_resource::<T>() {
            log::debug!("ResMut: Missing resource: {}", std::any::type_name::<T>());
            return false;
        }
        true
    }
}

unsafe impl<T: Resource> SystemParam for Option<ResMut<'_, T>> {
    type State = ();
    type Item<'w, 's> = Option<ResMut<'w, T>>;

    fn init_state(_: &mut World) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_written: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    /// # Safety
    ///
    /// Caller must ensure that the resource exists and that we have exclusive access to it
    unsafe fn fetch<'w, 's>(
        _: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        unsafe { world.get_resource_mut::<T>() }
    }

    fn can_run(_world: &World) -> bool {
        true
    }
}

macro_rules! impl_system_param_tuple {
    ($($param:ident),*) => {
        #[allow(unused, non_snake_case)]
        unsafe impl<$($param),*> SystemParam for ($($param),*)
        where
            $($param: SystemParam),*
        {
            type State = ($($param::State),*);
            type Item<'w, 's> = ($($param::Item<'w, 's>),*);

            fn init_state(world: &mut World) -> Self::State {
                ($($param::init_state(world)),*)
            }

            fn access() -> SystemAccess {
                let mut access = SystemAccess::default();

                $(
                    let a = $param::access();
                    assert!(a.is_compatible(&access), "SystemParam validation failed for {}", std::any::type_name::<$param>());
                    access.extend(a);
                )*

                access
            }

            unsafe fn fetch<'w, 's>(state: &'s mut Self::State, world: UnsafeWorldCell<'w>) -> Self::Item<'w, 's> {
                let ($($param),*) = state;
                unsafe { ($($param::fetch($param, world)),*) }
            }

            fn apply(state: &mut Self::State, world: &mut World) {
                let ($($param),*) = state;
                $(
                    $param::apply($param, world);
                )*
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

pub trait SystemParamFunction<M>: 'static + Send + Sync {
    type Param: SystemParam + 'static;
    type Output;

    fn run(&mut self, param: SystemParamItem<Self::Param>) -> Self::Output;
}

pub struct FunctionSystem<M, F>
where
    M: 'static,
    F: SystemParamFunction<M>,
{
    param_state: Option<SystemState<F::Param>>,
    func: F,
    _marker: std::marker::PhantomData<fn() -> M>,
}

impl<M, F> FunctionSystem<M, F>
where
    M: 'static,
    F: SystemParamFunction<M>,
{
    pub const fn new(func: F) -> Self {
        Self {
            param_state: None,
            func,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<M, F> System for FunctionSystem<M, F>
where
    M: 'static,
    F: SystemParamFunction<M>,
{
    type Output = F::Output;

    fn name(&self) -> &str {
        std::any::type_name::<F>()
    }

    fn initialize(&mut self, world: &mut World) {
        let param_state = SystemState {
            access: F::Param::access(),
            state: F::Param::init_state(world),
        };
        self.param_state = Some(param_state);
    }

    fn access(&self) -> SystemAccess {
        F::Param::access()
    }

    fn run(&mut self, world: &mut World) -> Self::Output {
        // validate access
        self.access();
        let param_state = self.param_state.as_mut().unwrap();

        // SAFETY:
        // - We have mutable access to the World.
        // - We have validated that the access is safe.
        let fetch =
            unsafe { F::Param::fetch(&mut param_state.state, world.as_unsafe_world_cell()) };
        let out = self.func.run(fetch);
        F::Param::apply(&mut param_state.state, world);
        out
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

    fn name(&self) -> &str {
        std::any::type_name::<F>()
    }

    fn into_system(self) -> Box<Self::System> {
        Box::new(FunctionSystem {
            param_state: None,
            func: self,
            _marker: std::marker::PhantomData,
        })
    }
}

macro_rules! impl_function_system {
    ($($param:ident),*) => {
        #[allow(unused, non_snake_case)]
        impl<Func, Output, $($param,)*> SystemParamFunction<fn($($param,)*)> for Func
        where for<'a> &'a mut Func:
            FnMut($($param),*) -> Output
            + FnMut($(SystemParamItem<$param>),*) -> Output,
            $($param: SystemParam + 'static),*,
            Func: 'static + Send + Sync,
        {
            type Param = ($($param),*);
            type Output = Output;

            fn run(&mut self, param: SystemParamItem<Self::Param>) -> Output {
                fn inner<Output, $($param,)*>(
                    mut func: impl FnMut($($param),*) -> Output,
                    param: ($($param),*),
                ) -> Output {
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

pub fn assert_is_system<Marker>(_: impl IntoSystem<Marker>) {}

#[derive(Default)]
pub struct SystemGraph {
    systems: StableDiGraph<SharedLock<Box<dyn System<Output = ()>>>, ()>,
    index_cache: FxHashMap<TypeId, NodeIndex>,
}

impl SystemGraph {
    /// Adds a system to the graph.
    pub fn add_system<M, S>(&mut self, system: S) -> NodeIndex
    where
        M: 'static,
        S: IntoSystem<M>,
        S::System: System<Output = ()>,
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
        S::System: System<Output = ()>,
        AFTER::System: System<Output = ()>,
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
        S::System: System<Output = ()>,
        BEFORE::System: System<Output = ()>,
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

    /// Runs all systems in the graph one-by-one in topological order, ensuring that all dependencies are respected.
    pub fn run_single_threaded(&mut self, world: &mut World) -> Result<()> {
        let mut schedule = Topo::new(&self.systems);
        while let Some(node) = schedule.next(&self.systems) {
            let system = &self.systems[node];
            if !system.read().can_run(world) {
                log::trace!("Skipping system: {}", system.read().name());
                continue;
            }
            log::trace!("Running system: {}", system.read().name());
            system.write().initialize(world);
            system.write().run(world);
        }

        Ok(())
    }
}
