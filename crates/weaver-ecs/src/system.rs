use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
};

use crate::{
    component::{Res, ResMut},
    prelude::{Resource, World},
};
use petgraph::{prelude::*, visit::Topo};
use weaver_util::{
    lock::SharedLock,
    prelude::{anyhow, Result},
    warn_once,
};

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
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
    fn access(&self) -> SystemAccess;
    #[allow(unused)]
    fn initialize(&mut self, world: &mut World) {}
    fn run_locked(&mut self, world: &mut World) -> Result<()>;
    #[allow(unused)]
    fn can_run(&self, world: &World) -> bool {
        true
    }
}

pub trait IntoSystem<Marker> {
    type System: System;

    fn into_system(self) -> Self::System;
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

    pub fn get<'w, 's>(&'s mut self, world: &'w World) -> SystemParamItem<'w, 's, P> {
        // validate access
        self.access();
        unsafe { P::fetch(&mut self.state, world) }
    }
}

impl<P: SystemParam> SystemParam for SystemState<P> {
    type State = P::State;
    type Item<'w, 's> = P::Item<'w, 's>;

    fn validate_access(access: &SystemAccess) -> bool {
        P::validate_access(access)
    }

    fn access() -> SystemAccess {
        P::access()
    }

    fn init_state(world: &mut World) -> Self::State {
        P::init_state(world)
    }

    unsafe fn fetch<'w, 's>(state: &'s mut Self::State, world: &'w World) -> Self::Item<'w, 's> {
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
    pub fn item(&self) -> &P::Item<'w, 's> {
        &self.item
    }

    pub fn into_inner(self) -> P::Item<'w, 's> {
        self.item
    }
}

impl<'w2, 's2, P: SystemParam> SystemParam for SystemParamWrapper<'w2, 's2, P> {
    type State = P::State;
    type Item<'w, 's> = P::Item<'w, 's>;

    fn validate_access(access: &SystemAccess) -> bool {
        P::validate_access(access)
    }

    fn access() -> SystemAccess {
        P::access()
    }

    fn init_state(world: &mut World) -> Self::State {
        P::init_state(world)
    }

    unsafe fn fetch<'w, 's>(state: &'s mut Self::State, world: &'w World) -> Self::Item<'w, 's> {
        unsafe { P::fetch(state, world) }
    }

    fn can_run(world: &World) -> bool {
        P::can_run(world)
    }
}

pub trait SystemParam {
    type State: Send + Sync;
    type Item<'w, 's>: SystemParam<State = Self::State>;

    fn validate_access(access: &SystemAccess) -> bool;

    fn access() -> SystemAccess;

    fn init_state(world: &mut World) -> Self::State;

    /// # Safety
    ///
    /// Caller must ensure that all system params being used are valid for simultaneous access.
    unsafe fn fetch<'w, 's>(state: &'s mut Self::State, world: &'w World) -> Self::Item<'w, 's>;

    #[allow(unused)]
    fn apply(state: &mut Self::State, world: &mut World) {}

    #[allow(unused)]
    fn can_run(world: &World) -> bool {
        true
    }
}

// NOTE: Not marking this as unsafe, unlike Bevy, since we don't actually violate memory safety in our implementation
// (Weaver lacks any form of `UnsafeWorldCell`)
pub trait ReadOnlySystemParam: SystemParam {}

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

impl<T: SystemParam> SystemParam for ParamSet<'_, '_, T> {
    type State = T::State;
    type Item<'w, 's> = T::Item<'w, 's>;

    fn validate_access(access: &SystemAccess) -> bool {
        T::validate_access(access)
    }

    fn access() -> SystemAccess {
        T::access()
    }

    fn init_state(world: &mut World) -> Self::State {
        T::init_state(world)
    }

    unsafe fn fetch<'w, 's>(state: &'s mut Self::State, world: &'w World) -> Self::Item<'w, 's> {
        unsafe { T::fetch(state, world) }
    }

    fn can_run(world: &World) -> bool {
        T::can_run(world)
    }
}

impl SystemParam for () {
    type State = ();
    type Item<'w, 's> = ();

    fn validate_access(_access: &SystemAccess) -> bool {
        true
    }

    fn init_state(_: &mut World) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: Vec::new(),
            resources_written: Vec::new(),
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    unsafe fn fetch<'w, 's>(_: &'s mut Self::State, _world: &'w World) -> Self::Item<'w, 's> {}

    fn can_run(_: &World) -> bool {
        true
    }
}

impl ReadOnlySystemParam for () {}

impl<T: Resource> SystemParam for Res<'_, T> {
    type State = ();
    type Item<'w, 's> = Res<'w, T>;

    fn validate_access(access: &SystemAccess) -> bool {
        if access.resources_written.contains(&TypeId::of::<T>()) {
            return false;
        }
        true
    }

    fn init_state(_: &mut World) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: vec![TypeId::of::<T>()],
            resources_written: Vec::new(),
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    /// # Safety
    ///
    /// Caller must ensure that the resource exists and that we have shared access to it
    unsafe fn fetch<'w, 's>(_: &'s mut Self::State, world: &'w World) -> Self::Item<'w, 's> {
        unsafe { world.get_resource_unsafe::<T>().unwrap() }
    }

    fn can_run(world: &World) -> bool {
        if !world.has_resource::<T>() {
            warn_once!("Res: Missing resource: {}", std::any::type_name::<T>());
            return false;
        }
        true
    }
}

impl<T: Resource> ReadOnlySystemParam for Res<'_, T> {}

impl<T: Resource> SystemParam for Option<Res<'_, T>> {
    type State = ();
    type Item<'w, 's> = Option<Res<'w, T>>;

    fn validate_access(access: &SystemAccess) -> bool {
        if access.resources_written.contains(&TypeId::of::<T>()) {
            return false;
        }
        true
    }

    fn init_state(_: &mut World) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: vec![TypeId::of::<T>()],
            resources_written: Vec::new(),
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    /// # Safety
    ///
    /// Caller must ensure that the resource exists and that we have shared access to it
    unsafe fn fetch<'w, 's>(_: &'s mut Self::State, world: &'w World) -> Self::Item<'w, 's> {
        unsafe { world.get_resource_unsafe::<T>() }
    }

    fn can_run(_world: &World) -> bool {
        true
    }
}

impl<T: Resource> SystemParam for ResMut<'_, T> {
    type State = ();
    type Item<'w, 's> = ResMut<'w, T>;

    fn validate_access(access: &SystemAccess) -> bool {
        if access.resources_read.contains(&TypeId::of::<T>())
            || access.resources_written.contains(&TypeId::of::<T>())
        {
            return false;
        }
        true
    }

    fn init_state(_: &mut World) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: Vec::new(),
            resources_written: vec![TypeId::of::<T>()],
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    /// # Safety
    ///
    /// Caller must ensure that the resource exists and that we have exclusive access to it
    unsafe fn fetch<'w, 's>(_: &'s mut Self::State, world: &'w World) -> Self::Item<'w, 's> {
        unsafe { world.get_resource_mut_unsafe::<T>().unwrap() }
    }

    fn can_run(world: &World) -> bool {
        if !world.has_resource::<T>() {
            warn_once!("ResMut: Missing resource: {}", std::any::type_name::<T>());
            return false;
        }
        true
    }
}

impl<T: Resource> ReadOnlySystemParam for ResMut<'_, T> {}

impl<T: Resource> SystemParam for Option<ResMut<'_, T>> {
    type State = ();
    type Item<'w, 's> = Option<ResMut<'w, T>>;

    fn validate_access(access: &SystemAccess) -> bool {
        if access.resources_read.contains(&TypeId::of::<T>())
            || access.resources_written.contains(&TypeId::of::<T>())
        {
            return false;
        }
        true
    }

    fn init_state(_: &mut World) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: Vec::new(),
            resources_written: vec![TypeId::of::<T>()],
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    /// # Safety
    ///
    /// Caller must ensure that the resource exists and that we have exclusive access to it
    unsafe fn fetch<'w, 's>(_: &'s mut Self::State, world: &'w World) -> Self::Item<'w, 's> {
        unsafe { world.get_resource_mut_unsafe::<T>() }
    }

    fn can_run(_world: &World) -> bool {
        true
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
            type Item<'w, 's> = ($($param::Item<'w, 's>),*);

            fn validate_access(access: &SystemAccess) -> bool {
                $(
                    if !$param::validate_access(access) {
                        return false;
                    }
                )*

                true
            }

            fn init_state(world: &mut World) -> Self::State {
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
                    assert!($param::validate_access(&access), "SystemParam validation failed");
                    access.extend($param::access());
                )*

                access
            }

            unsafe fn fetch<'w, 's>(state: &'s mut Self::State, world: &'w World) -> Self::Item<'w, 's> {
                let ($($param),*) = state;
                unsafe { ($($param::fetch($param, world)),*) }
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
    type Param: SystemParam + 'static;

    fn run(&mut self, param: SystemParamItem<Self::Param>) -> Result<()>;
}

pub struct FunctionSystem<M, F>
where
    F: SystemParamFunction<M>,
{
    param_state: Option<SystemState<F::Param>>,
    func: F,
    _marker: std::marker::PhantomData<fn() -> M>,
}

impl<M, F> FunctionSystem<M, F>
where
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

    fn run_locked(&mut self, world: &mut World) -> Result<()> {
        // validate access
        self.access();
        let param_state = self.param_state.as_mut().unwrap();
        // SAFETY:
        // - We have mutable access to the World.
        // - We have validated that the access is safe.

        let fetch = unsafe { F::Param::fetch(&mut param_state.state, world) };
        self.func.run(fetch)?;
        F::Param::apply(&mut param_state.state, world);
        Ok(())
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

    fn into_system(self) -> Self::System {
        FunctionSystem {
            param_state: None,
            func: self,
            _marker: std::marker::PhantomData,
        }
    }
}

macro_rules! impl_function_system {
    ($($param:ident),*) => {
        #[allow(unused, non_snake_case)]
        impl<Func, $($param,)*> SystemParamFunction<fn($($param,)*)> for Func
        where for<'a> &'a mut Func:
            FnMut($($param),*) -> Result<()>
            + FnMut($(SystemParamItem<$param>),*) -> Result<()>,
            $($param: SystemParam + 'static),*,
            Func: Send + Sync + 'static,
        {
            type Param = ($($param),*);

            fn run(&mut self, param: SystemParamItem<Self::Param>) -> Result<()> {
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

pub trait ExclusiveSystemParam {
    type State: Send + Sync;
    type Item<'w, 's>: ExclusiveSystemParam<State = Self::State>;

    fn init_state(world: &mut World) -> Self::State;

    fn fetch<'w, 's>(state: &'s mut Self::State, world: &'w mut World) -> Self::Item<'w, 's>;

    #[allow(unused)]
    fn can_run(world: &World) -> bool {
        true
    }
}

pub type ExclusiveSystemParamItem<'w, 's, P> = <P as ExclusiveSystemParam>::Item<'w, 's>;

impl ExclusiveSystemParam for &mut World {
    type State = ();
    type Item<'w, 's> = &'w mut World;

    fn init_state(_: &mut World) -> Self::State {}

    fn fetch<'w, 's>(_: &'s mut Self::State, world: &'w mut World) -> Self::Item<'w, 's> {
        world
    }

    fn can_run(_: &World) -> bool {
        true
    }
}

pub trait ExclusiveSystemParamFunction<M>: Send + Sync + 'static {
    type Param: ExclusiveSystemParam + Send + Sync + 'static;
    fn run(&mut self, param: ExclusiveSystemParamItem<Self::Param>) -> Result<()>;
}

impl<F> ExclusiveSystemParamFunction<fn(&mut World)> for F
where
    F: FnMut(&mut World) -> Result<()> + Send + Sync + 'static,
{
    type Param = &'static mut World;

    fn run(&mut self, world: &mut World) -> Result<()> {
        self(world)
    }
}

pub struct ExclusiveSystemState<P: ExclusiveSystemParam> {
    state: P::State,
    _phantom: std::marker::PhantomData<P>,
}

impl<P: ExclusiveSystemParam> ExclusiveSystemState<P> {
    pub fn new(world: &mut World) -> Self {
        Self {
            state: P::init_state(world),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn get<'w, 's>(&'s mut self, world: &'w mut World) -> ExclusiveSystemParamItem<'w, 's, P> {
        P::fetch(&mut self.state, world)
    }
}

pub struct ExclusiveFunctionSystem<M, F>
where
    M: 'static,
    F: ExclusiveSystemParamFunction<M>,
{
    func: F,
    param: Option<ExclusiveSystemState<F::Param>>,
    _marker: std::marker::PhantomData<fn() -> M>,
}

impl<M, F> ExclusiveFunctionSystem<M, F>
where
    M: 'static,
    F: ExclusiveSystemParamFunction<M>,
{
    pub const fn new(func: F) -> Self {
        Self {
            func,
            param: None,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<M, F> System for ExclusiveFunctionSystem<M, F>
where
    M: 'static,
    F: ExclusiveSystemParamFunction<M>,
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

    fn initialize(&mut self, world: &mut World) {
        let param = ExclusiveSystemState {
            state: F::Param::init_state(world),
            _phantom: std::marker::PhantomData,
        };
        self.param = Some(param);
    }

    fn run_locked(&mut self, world: &mut World) -> Result<()> {
        let param = self.param.as_mut().unwrap();
        let item = param.get(world);
        self.func.run(item)
    }
}

pub struct ExclusiveFunctionSystemMarker;

impl<M, F> IntoSystem<(ExclusiveFunctionSystemMarker, M)> for F
where
    M: 'static,
    F: ExclusiveSystemParamFunction<M>,
{
    type System = ExclusiveFunctionSystem<M, F>;

    fn into_system(self) -> Self::System {
        ExclusiveFunctionSystem {
            func: self,
            param: None,
            _marker: std::marker::PhantomData,
        }
    }
}

pub fn assert_is_system<Marker>(_: impl IntoSystem<Marker>) {}
pub fn assert_is_non_exclusive_system<Marker, S>(system: S)
where
    Marker: 'static,
    S: SystemParamFunction<Marker>,
    S::Param: ReadOnlySystemParam,
{
    assert_is_system(system)
}

#[derive(Default)]
pub struct SystemGraph {
    systems: StableDiGraph<SharedLock<Box<dyn System>>, ()>,
    index_cache: HashMap<TypeId, NodeIndex>,
}

impl SystemGraph {
    pub fn add_system<M, S>(&mut self, system: S) -> NodeIndex
    where
        S: IntoSystem<M> + 'static,
    {
        let node = self
            .systems
            .add_node(SharedLock::new(Box::new(system.into_system())));
        self.index_cache.insert(TypeId::of::<S>(), node);
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
    }

    pub fn add_system_after<M1, M2, S1, S2>(&mut self, system: S1, _after: S2)
    where
        S1: IntoSystem<M1> + 'static,
        S2: IntoSystem<M2> + 'static,
    {
        let node = self.add_system(system);
        let parent = self.index_cache[&TypeId::of::<S2>()];
        self.systems.add_edge(parent, node, ());
    }

    pub fn add_system_before<M1, M2, S1, S2>(&mut self, system: S1, _before: S2)
    where
        S1: IntoSystem<M1> + 'static,
        S2: IntoSystem<M2> + 'static,
    {
        let node = self.add_system(system);
        let child = self.index_cache[&TypeId::of::<S2>()];
        self.systems.add_edge(node, child, ());
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

    pub fn run(&mut self, world: &mut World) -> Result<()> {
        let mut schedule = Topo::new(&self.systems);
        while let Some(node) = schedule.next(&self.systems) {
            let system = &mut self.systems[node];
            if !system.read().can_run(world) {
                continue;
            }
            system.write().initialize(world);
            system.write().run_locked(world)?;
        }

        Ok(())
    }
}
