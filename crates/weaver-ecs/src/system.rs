use std::{any::TypeId, collections::VecDeque, sync::Arc};

use crate::{
    change_detection::{Tick, TickRange},
    component::{Component, Res, ResMut},
    condition::{BoxedCondition, Condition},
    prelude::World,
    world::ConstructFromWorld,
};
use petgraph::prelude::*;
use weaver_util::prelude::*;

/// A system access descriptor, indicating what resources and components a system reads and writes. This is used to validate system access at runtime.
#[derive(Default, Clone)]
pub struct SystemAccess {
    pub resources_read: FxHashSet<TypeId>,
    pub resources_written: FxHashSet<TypeId>,
    pub components_read: FxHashSet<TypeId>,
    pub components_written: FxHashSet<TypeId>,
    /// May touch anything in the world (a system taking `&World`).
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
    pub fn is_compatible(&self, other: &Self) -> bool {
        if self.exclusive || other.exclusive {
            return false;
        }

        if !self.components_read.is_disjoint(&other.components_written) {
            return false;
        }

        if !self.components_written.is_disjoint(&other.components_read) {
            return false;
        }

        if !self
            .components_written
            .is_disjoint(&other.components_written)
        {
            return false;
        }

        if !self.resources_read.is_disjoint(&other.resources_written) {
            return false;
        }

        if !self.resources_written.is_disjoint(&other.resources_read) {
            return false;
        }

        if !self.resources_written.is_disjoint(&other.resources_written) {
            return false;
        }

        true
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

    /// Runs the system on the world. `ticks.this_run` is the tick this run stamps its changes
    /// with, and `ticks.last_run` is the `this_run` of the system's previous run.
    fn run(&mut self, world: &World, ticks: TickRange);

    #[allow(unused)]
    fn apply_deferred(&mut self, world: &mut World) {}

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

    /// Fetches the parameter. `ticks` are the running system's change ticks: changes stamped
    /// after `ticks.last_run` count as changed, and writes are stamped with `ticks.this_run`.
    fn fetch(world: &World, state: &Self::State, ticks: TickRange) -> Self::Item;

    #[allow(unused)]
    fn apply(state: &mut Self::State, world: &mut World) {}

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

    fn fetch(_: &World, _: &Self::State, _: TickRange) -> Self::Item {}

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

    pub fn get(&self) -> OwnedRead<T> {
        self.value.read()
    }

    pub fn get_mut(&mut self) -> OwnedWrite<T> {
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

    fn fetch(_world: &World, state: &Self::State, _ticks: TickRange) -> Self::Item {
        state.clone()
    }
}

impl<T: Component> SystemParam for Res<T> {
    type Item = Res<T>;
    type State = ();

    fn access() -> SystemAccess {
        SystemAccess {
            resources_read: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _: &Self::State, ticks: TickRange) -> Self::Item {
        world.get_resource_with_ticks::<T>(ticks).unwrap()
    }

    fn can_run(world: &World) -> bool {
        if !world.has_resource::<T>() {
            log::debug!("Res: Resource {:?} does not exist", T::type_name());
            return false;
        }
        if world.get_resource::<T>().is_none() {
            log::debug!("Res: Resource {:?} is already borrowed", T::type_name());
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
            resources_read: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _: &Self::State, ticks: TickRange) -> Self::Item {
        world.get_resource_with_ticks::<T>(ticks)
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
            resources_written: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _: &Self::State, ticks: TickRange) -> Self::Item {
        world.get_resource_mut_with_ticks::<T>(ticks).unwrap()
    }
    fn can_run(world: &World) -> bool {
        if !world.has_resource::<T>() {
            log::debug!("ResMut: Resource {:?} does not exist", T::type_name());
            return false;
        }
        if world.get_resource_mut::<T>().is_none() {
            log::debug!("ResMut: Resource {:?} is already borrowed", T::type_name());
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
            resources_written: FxHashSet::from_iter([TypeId::of::<T>()]),
            ..Default::default()
        }
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _: &Self::State, ticks: TickRange) -> Self::Item {
        world.get_resource_mut_with_ticks::<T>(ticks)
    }

    fn can_run(_world: &World) -> bool {
        true
    }
}

macro_rules! impl_system_param_tuple {
    ($( $param:ident ),*) => {
        #[allow(non_snake_case)]
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

            fn update_state(state: &mut Self::State, world: &World) {
                let ($( $param, )*) = state;
                $( $param::update_state($param, world); )*
            }

            fn fetch(world: &World, state: &Self::State, ticks: TickRange) -> Self::Item {
                let ($( $param, )*) = state;
                ($( $param::fetch(world, $param, ticks), )*)
            }

            fn apply(state: &mut Self::State, world: &mut World) {
                let ($( $param, )*) = state;
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

pub struct SystemParamWrapper<S: SystemParam>(S::Item);

impl<S: SystemParam> SystemParamWrapper<S> {
    pub fn new(item: S::Item) -> Self {
        Self(item)
    }

    pub fn item(&self) -> &S::Item {
        &self.0
    }

    pub fn item_mut(&mut self) -> &mut S::Item {
        &mut self.0
    }

    pub fn into_item(self) -> S::Item {
        self.0
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

    fn update_state(state: &mut Self::State, world: &World) {
        <S as SystemParam>::update_state(state, world);
    }

    fn fetch(world: &World, state: &Self::State, ticks: TickRange) -> Self::Item {
        Self(<S as SystemParam>::fetch(world, state, ticks))
    }

    fn apply(state: &mut Self::State, world: &mut World) {
        <S as SystemParam>::apply(state, world);
    }

    fn can_run(world: &World) -> bool {
        S::can_run(world)
    }
}

pub trait SystemParamFunction<M>: 'static + Send + Sync {
    type Param: SystemParam + 'static;

    fn init_state(world: &World) -> SystemParamState<Self::Param>;

    fn update_state(state: &mut SystemParamState<Self::Param>, world: &World);

    fn run(&self, param: SystemParamItem<Self::Param>);
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

    fn run(&mut self, world: &World, ticks: TickRange) {
        let state = self.state.as_mut().expect("State not initialized");
        F::update_state(state, world);
        let fetch = F::Param::fetch(world, state, ticks);
        let func = self.func.clone();
        func.run(fetch);
    }

    fn apply_deferred(&mut self, world: &mut World) {
        let state = self.state.as_mut().expect("State not initialized");
        F::Param::apply(state, world);
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
        Box::new(FunctionSystem::new(self))
    }
}

macro_rules! impl_function_system {
    ($($param:ident),*) => {
        #[allow(unused, non_snake_case)]
        impl<Func, $($param,)*> SystemParamFunction<fn($($param,)*)> for Func
        where for<'a> &'a Func:
            Fn($($param),*)
            + Fn($(SystemParamItem<$param>),*) ,
            $($param: SystemParam + 'static),*,
            Func: 'static + Send + Sync,
        {
            type Param = ($($param),*);

            fn init_state(world: &World) -> SystemParamState<Self::Param> {
                ($($param::init_state(world)),*)
            }

            fn update_state(state: &mut SystemParamState<Self::Param>, world: &World) {
                let ($($param),*) = state;
                $($param::update_state($param, world);)*
            }

            fn run(&self, param: SystemParamItem<Self::Param>) {
                fn inner<$($param,)*>(
                    mut func: impl Fn($($param),*),
                    param: ($($param),*),
                )
                {
                    let ($($param),*) = param;
                    func($($param),*);
                }

                let ($($param),*) = param;
                inner(self, ($($param),*));
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

/// A system that takes the whole world, for work that needs to reach arbitrary resources and
/// components at runtime (for example the renderer's per-view driver, which runs draw functions
/// registered by other plugins). Write it as `fn my_system(world: &World)`.
///
/// It conflicts with every other system, so the scheduler never overlaps it with anything. Changes
/// made through `world` are stamped with the world's current tick.
pub struct WorldSystem<F> {
    func: F,
    access: SystemAccess,
}

impl<F> System for WorldSystem<F>
where
    F: Fn(&World) + Send + Sync + 'static,
{
    type Input = ();
    type Output = ();

    fn name(&self) -> &str {
        std::any::type_name::<F>()
    }

    fn access(&self) -> &SystemAccess {
        &self.access
    }

    fn run(&mut self, world: &World, _ticks: TickRange) {
        (self.func)(world);
    }
}

pub struct WorldSystemMarker;

impl<F> IntoSystem<WorldSystemMarker> for F
where
    F: Fn(&World) + Send + Sync + 'static,
{
    type System = WorldSystem<F>;

    fn into_system(self) -> Box<Self::System> {
        Box::new(WorldSystem {
            func: self,
            access: SystemAccess {
                exclusive: true,
                ..Default::default()
            },
        })
    }
}

/// Fetches a [`SystemParam`] from a `&World` outside the scheduler, keeping the parameter's state
/// between fetches. Change detection compares against the previous fetch.
pub struct SystemState<P: SystemParam> {
    state: P::State,
    last_run: Option<Tick>,
}

impl<P: SystemParam> SystemState<P> {
    pub fn new(world: &World) -> Self {
        Self {
            state: P::init_state(world),
            last_run: None,
        }
    }

    /// Fetches the parameter, or returns `None` if it isn't available (for example a missing
    /// resource behind a `Res`).
    pub fn get(&mut self, world: &World) -> Option<P::Item> {
        if !P::can_run(world) {
            return None;
        }
        P::update_state(&mut self.state, world);
        let this_run = world.bump_change_tick();
        let last_run = self.last_run.unwrap_or_else(|| Tick::never_run(this_run));
        self.last_run = Some(this_run);
        Some(P::fetch(
            world,
            &self.state,
            TickRange::new(last_run, this_run),
        ))
    }

    /// Applies deferred work (such as commands) recorded by the fetched parameters.
    pub fn apply(&mut self, world: &mut World) {
        P::apply(&mut self.state, world);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemAddOption {
    After(TypeId, &'static str),
    Before(TypeId, &'static str),
}

pub struct SystemConfig {
    system_type_id: TypeId,
    system: Box<dyn System<Input = (), Output = ()>>,
    options: Vec<SystemAddOption>,
    conditions: Vec<BoxedCondition>,
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
            options: Vec::new(),
            conditions: Vec::new(),
        }
    }

    pub fn after<M2: 'static, T: IntoSystem<M2>>(mut self, _system: T) -> Self {
        let id = TypeId::of::<T>();
        if self
            .options
            .iter()
            .any(|o| matches!(o, SystemAddOption::Before(other, _) if *other == id))
        {
            panic!("Cannot add system both after and before another system");
        }
        self.options
            .push(SystemAddOption::After(id, std::any::type_name::<T>()));
        self
    }

    pub fn before<M2: 'static, T: IntoSystem<M2>>(mut self, _system: T) -> Self {
        let id = TypeId::of::<T>();
        if self
            .options
            .iter()
            .any(|o| matches!(o, SystemAddOption::After(other, _) if *other == id))
        {
            panic!("Cannot add system both after and before another system");
        }
        self.options
            .push(SystemAddOption::Before(id, std::any::type_name::<T>()));
        self
    }

    /// Only runs the system when `condition` holds. Several conditions must all hold.
    pub fn run_if(mut self, condition: impl Condition) -> Self {
        self.conditions.push(Box::new(condition));
        self
    }
}

pub trait IntoSystemConfig<M>: Sized + 'static {
    fn finish(self) -> SystemConfig;

    /// Runs this system after `system`. `system` may be registered later in the same stage; it
    /// has to exist by the time the stage is initialized.
    fn after<M2: 'static, T: IntoSystem<M2>>(self, system: T) -> SystemConfig {
        self.finish().after(system)
    }

    /// Runs this system before `system`. `system` may be registered later in the same stage; it
    /// has to exist by the time the stage is initialized.
    fn before<M2: 'static, T: IntoSystem<M2>>(self, system: T) -> SystemConfig {
        self.finish().before(system)
    }

    /// Only runs the system when `condition` holds. See [`crate::condition`].
    fn run_if(self, condition: impl Condition) -> SystemConfig {
        self.finish().run_if(condition)
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

/// How many runs in a row a system without run conditions may be skipped (because a parameter
/// such as `Res<T>` isn't available) before a warning is logged.
const SKIP_WARNING_THRESHOLD: u32 = 100;

struct SystemNode {
    system: Box<dyn System<Input = (), Output = ()>>,
    conditions: Vec<BoxedCondition>,
    initialized: bool,
    /// `this_run` of the last time the system ran. `None` until it first runs.
    last_run: Option<Tick>,
    /// Consecutive runs skipped because `can_run` was false.
    skipped_in_a_row: u32,
    warned_about_skipping: bool,
}

impl SystemNode {
    fn name(&self) -> &str {
        self.system.name()
    }

    /// Evaluates the run conditions against the system's own last-run tick.
    fn conditions_hold(&mut self, world: &World) -> bool {
        if self.conditions.is_empty() {
            return true;
        }
        let this_run = world.read_change_tick();
        let ticks = TickRange::new(
            self.last_run.unwrap_or_else(|| Tick::never_run(this_run)),
            this_run,
        );
        self.conditions
            .iter_mut()
            .all(|condition| condition.evaluate(world, ticks))
    }

    fn run(&mut self, world: &World) {
        let this_run = world.bump_change_tick();
        let last_run = self.last_run.unwrap_or_else(|| Tick::never_run(this_run));
        self.system.run(world, TickRange::new(last_run, this_run));
        self.last_run = Some(this_run);
        self.skipped_in_a_row = 0;
    }

    fn note_skipped(&mut self) {
        log::debug!("Skipping system: {}", self.name());
        self.skipped_in_a_row = self.skipped_in_a_row.saturating_add(1);
        if self.conditions.is_empty()
            && self.skipped_in_a_row >= SKIP_WARNING_THRESHOLD
            && !self.warned_about_skipping
        {
            self.warned_about_skipping = true;
            log::warn!(
                "System {} has been skipped {} runs in a row because one of its parameters is unavailable \
                 (usually a missing resource). If that is intended, gate it explicitly with \
                 `.run_if(resource_exists::<T>())`.",
                self.name(),
                self.skipped_in_a_row,
            );
        }
    }
}

/// An ordering edge whose other end may not have been registered yet.
struct PendingEdge {
    from: TypeId,
    from_name: &'static str,
    to: TypeId,
    to_name: &'static str,
}

#[derive(Default)]
pub struct SystemGraph {
    systems: StableDiGraph<SystemNode, ()>,
    index_cache: TypeIdMap<NodeIndex>,
    pending_edges: Vec<PendingEdge>,
}

impl SystemGraph {
    /// Adds a system to the graph. Adding a system that is already in the graph does nothing.
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
            conditions,
        } = config;

        // if the system already exists in the graph, don't add it twice
        if let Some(&node) = self.index_cache.get(&system_type_id) {
            log::warn!(
                "System {} was added to the same stage twice; ignoring the second one",
                system.name()
            );
            return node;
        }

        let name = std::any::type_name::<S>();
        let node = self.systems.add_node(SystemNode {
            system,
            conditions,
            initialized: false,
            last_run: None,
            skipped_in_a_row: 0,
            warned_about_skipping: false,
        });
        self.index_cache.insert(system_type_id, node);

        for option in options {
            let edge = match option {
                SystemAddOption::After(id, other_name) => PendingEdge {
                    from: id,
                    from_name: other_name,
                    to: system_type_id,
                    to_name: name,
                },
                SystemAddOption::Before(id, other_name) => PendingEdge {
                    from: system_type_id,
                    from_name: name,
                    to: id,
                    to_name: other_name,
                },
            };
            self.pending_edges.push(edge);
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
        self.pending_edges.push(PendingEdge {
            from: TypeId::of::<S>(),
            from_name: std::any::type_name::<S>(),
            to: TypeId::of::<T>(),
            to_name: std::any::type_name::<T>(),
        });
    }

    pub fn remove_system<M, S>(&mut self, _system: S)
    where
        M: 'static,
        S: IntoSystem<M>,
    {
        let system_type_id = TypeId::of::<S>();
        if let Some(&node) = self.index_cache.get(&system_type_id) {
            self.systems.remove_node(node);
            self.index_cache.remove(&system_type_id);
        }
        self.pending_edges
            .retain(|edge| edge.from != system_type_id && edge.to != system_type_id);
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

    /// Turns `.after()`/`.before()` requests into edges. Fails if a referenced system was never
    /// added to this stage.
    fn resolve_pending_edges(&mut self) -> Result<()> {
        for edge in std::mem::take(&mut self.pending_edges) {
            let (Some(&from), Some(&to)) = (
                self.index_cache.get(&edge.from),
                self.index_cache.get(&edge.to),
            ) else {
                let missing = if self.index_cache.contains_key(&edge.from) {
                    edge.to_name
                } else {
                    edge.from_name
                };
                bail!(
                    "System ordering {} -> {} refers to {}, which isn't in this stage",
                    edge.from_name,
                    edge.to_name,
                    missing
                );
            };
            if !self.systems.contains_edge(from, to) {
                self.systems.add_edge(from, to, ());
            }
        }
        Ok(())
    }

    fn check_for_cycles(&self) -> Result<()> {
        if !petgraph::algo::is_cyclic_directed(&self.systems) {
            return Ok(());
        }
        let mut cycles = Vec::new();
        for scc in petgraph::algo::kosaraju_scc(&self.systems) {
            if scc.len() > 1 {
                let names = scc
                    .iter()
                    .map(|&node| self.systems[node].name().to_string())
                    .collect::<Vec<_>>();
                cycles.push(names);
            }
        }
        bail!("Cyclic system dependency detected: {:#?}", cycles)
    }

    /// Resolves system dependencies, ensuring that no system mutably accesses the same resource or component at the same time as another system.
    pub fn resolve_dependencies(&mut self) -> Result<()> {
        self.resolve_pending_edges()?;
        self.check_for_cycles()?;

        // Order every conflicting pair that shares a layer. Two systems in the same layer can't
        // reach each other, so an edge between them never creates a cycle; each pass adds at
        // least one edge, so this terminates.
        loop {
            let mut added = false;
            for layer in self.get_batches() {
                for i in 0..layer.len() {
                    for j in i + 1..layer.len() {
                        let (node, other) = (layer[i], layer[j]);
                        let compatible = self.systems[node]
                            .system
                            .access()
                            .is_compatible(self.systems[other].system.access());
                        if !compatible {
                            self.systems.add_edge(node, other, ());
                            added = true;
                            break;
                        }
                    }
                    if added {
                        break;
                    }
                }
                if added {
                    break;
                }
            }
            if !added {
                return Ok(());
            }
        }
    }

    pub fn initialize(&mut self, world: &mut World) {
        if let Err(e) = self.resolve_dependencies() {
            panic!("{e}");
        }

        for node in self.systems.node_indices().collect::<Vec<_>>() {
            let node = &mut self.systems[node];
            if !node.initialized {
                node.system.initialize(world);
                node.initialized = true;
            }
        }
    }

    /// Runs all systems in the graph.
    pub fn run(&mut self, world: &mut World) -> Result<()> {
        if !self.pending_edges.is_empty()
            || self.systems.node_weights().any(|node| !node.initialized)
        {
            // systems were added after the stage was initialized
            self.initialize(world);
        }

        let schedule = self.get_batches();
        for layer in schedule {
            let mut skipped = Vec::new();
            let mut ran = Vec::new();
            for node in layer {
                let system = &mut self.systems[node];
                if !system.conditions_hold(world) {
                    continue;
                }
                if !system.system.can_run(world) {
                    skipped.push(node);
                    continue;
                }
                system.run(world);

                ran.push(node);
            }

            // every system in the layer has finished (and dropped its params), so nothing holds a loan into the world.
            // Each run bumped the world tick past its own `this_run`, so changes made by commands are newer than every run in this layer.
            for node in ran {
                self.systems[node].system.apply_deferred(world);
            }

            // run skipped systems synchronously, in case there was a resource conflict
            // this has to come after the apply above, so a system skipped for a missing resource can see one inserted by this layer's commands
            for node in skipped {
                let system = &mut self.systems[node];
                if !system.system.can_run(world) {
                    system.note_skipped();
                    continue;
                }

                system.run(world);

                // apply right away, so the remaining skipped systems see this one's commands too
                system.system.apply_deferred(world);
            }
        }

        Ok(())
    }
}
