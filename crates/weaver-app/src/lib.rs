use std::any::TypeId;

use plugin::Plugin;
use rustc_hash::FxHashMap;

use weaver_ecs::{
    change::WorldTicks,
    component::{Res, ResMut, Resource},
    reflect::registry::{TypeRegistry, Typed},
    system::IntoSystem,
    system_schedule::SystemStage,
    world::World,
};
use weaver_event::{Event, Events, ManuallyUpdatedEvents};
use weaver_util::{lock::SharedLock, prelude::Result};

pub mod plugin;

pub mod prelude {
    pub use crate::{
        plugin::Plugin, App, FinishFrame, Init, PostUpdate, PreUpdate, PrepareFrame, Shutdown,
        SubApp, Update,
    };
}

pub struct Init;
impl SystemStage for Init {}

pub struct PrepareFrame;
impl SystemStage for PrepareFrame {}

pub struct PreUpdate;
impl SystemStage for PreUpdate {}
pub struct Update;
impl SystemStage for Update {}
pub struct PostUpdate;
impl SystemStage for PostUpdate {}

pub struct FinishFrame;
impl SystemStage for FinishFrame {}

pub struct Shutdown;
impl SystemStage for Shutdown {}

pub trait Runner: 'static {
    fn run(&self, app: &mut App) -> Result<()>;
}

impl<T> Runner for T
where
    T: Fn(&mut App) -> Result<()> + Send + Sync + 'static,
{
    fn run(&self, app: &mut App) -> Result<()> {
        self(app)
    }
}

pub type ExtractFn = Box<dyn Fn(&mut World, &mut World) -> Result<()> + Send + Sync>;

pub trait AppLabel: 'static {}

pub struct SubApp {
    world: World,
    plugins: SharedLock<Vec<Box<dyn Plugin>>>,
    extract_fn: Option<ExtractFn>,
}

impl Default for SubApp {
    fn default() -> Self {
        let world = World::new();

        Self {
            world,
            plugins: SharedLock::new(Vec::default()),
            extract_fn: None,
        }
    }
}

impl SubApp {
    pub fn new() -> Self {
        Self::default()
    }

    fn as_app<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut App) -> R,
    {
        let mut app = App::empty();
        std::mem::swap(app.main_app_mut(), self);
        let result = f(&mut app);
        std::mem::swap(app.main_app_mut(), self);
        result
    }

    pub fn finish_plugins(&mut self) {
        for plugin in self.plugins.read_arc().iter() {
            self.as_app(|app| {
                log::debug!("Finishing plugin: {:?}", plugin.name());
                plugin.finish(app)
            })
            .unwrap();
        }
    }

    pub fn insert_resource<T: Resource>(&mut self, resource: T) -> &Self {
        self.world.insert_resource(resource);
        self
    }

    /// # Safety
    ///
    /// Caller ensures that there are no mutable references to the resource.
    pub unsafe fn get_resource_unsafe<T: Resource>(&self) -> Option<Res<T>> {
        unsafe { self.world.get_resource_unsafe::<T>() }
    }

    /// # Safety
    ///
    /// Caller ensures that there are no references to the resource, mutable or otherwise.
    pub unsafe fn get_resource_mut_unsafe<T: Resource>(&self) -> Option<ResMut<T>> {
        unsafe { self.world.get_resource_mut_unsafe::<T>() }
    }

    pub fn get_resource<T: Resource>(&mut self) -> Option<Res<T>> {
        self.world.get_resource::<T>()
    }

    pub fn get_resource_mut<T: Resource>(&mut self) -> Option<ResMut<T>> {
        self.world.get_resource_mut::<T>()
    }

    pub fn has_resource<T: Resource>(&self) -> bool {
        self.world.has_resource::<T>()
    }

    pub fn add_plugin<T: Plugin>(&mut self, plugin: T) -> Result<&mut Self> {
        if self
            .plugins
            .read_arc()
            .iter()
            .any(|plugin| (**plugin).type_id() == TypeId::of::<T>())
        {
            log::warn!("Plugin already added: {:?}", plugin.name());
            return Ok(self);
        }

        log::debug!("Adding plugin: {:?}", plugin.name());
        self.as_app(|app| plugin.build(app))?;

        self.plugins.write_arc().push(Box::new(plugin));

        Ok(self)
    }

    pub fn push_init_stage<T: SystemStage>(&mut self) -> &mut Self {
        self.world.push_init_stage::<T>();
        self
    }

    pub fn push_update_stage<T: SystemStage>(&mut self) -> &mut Self {
        self.world.push_update_stage::<T>();
        self
    }

    pub fn push_shutdown_stage<T: SystemStage>(&mut self) -> &mut Self {
        self.world.push_shutdown_stage::<T>();
        self
    }

    pub fn push_manual_stage<T: SystemStage>(&mut self) -> &mut Self {
        self.world.push_manual_stage::<T>();
        self
    }

    pub fn add_update_stage_before<T: SystemStage, U: SystemStage>(&mut self) -> &mut Self {
        self.world.add_stage_before::<T, U>();
        self
    }

    pub fn add_update_stage_after<T: SystemStage, U: SystemStage>(&mut self) -> &mut Self {
        self.world.add_stage_after::<T, U>();
        self
    }

    pub fn add_system<S: SystemStage, M>(
        &mut self,
        system: impl IntoSystem<M> + 'static,
        stage: S,
    ) -> &mut Self {
        self.world.add_system(system, stage);
        self
    }

    pub fn add_system_before<S: SystemStage, M1, M2>(
        &mut self,
        system: impl IntoSystem<M1> + 'static,
        before: impl IntoSystem<M2> + 'static,
        stage: S,
    ) -> &mut Self {
        self.world.add_system_before(system, before, stage);
        self
    }

    pub fn add_system_after<S: SystemStage, M1, M2>(
        &mut self,
        system: impl IntoSystem<M1> + 'static,
        after: impl IntoSystem<M2> + 'static,
        stage: S,
    ) -> &mut Self {
        self.world.add_system_after(system, after, stage);
        self
    }

    pub fn set_extract(&mut self, extract: ExtractFn) -> &mut Self {
        self.extract_fn = Some(extract);
        self
    }

    pub fn extract_fn(&self) -> Option<&ExtractFn> {
        self.extract_fn.as_ref()
    }

    pub fn read_world(&self) -> &World {
        &self.world
    }

    pub fn write_world(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn extract_from(&mut self, from: &mut World) -> Result<()> {
        if let Some(extract_fn) = self.extract_fn.as_mut() {
            extract_fn(from, &mut self.world)
        } else {
            Ok(())
        }
    }

    pub fn init(&mut self) {
        self.world.init().unwrap();
    }

    pub fn update(&mut self) {
        self.world.update().unwrap();
    }

    pub fn shutdown(&mut self) {
        self.world.shutdown().unwrap();
    }
}

pub struct SubApps {
    pub main: SubApp,
    sub_apps: FxHashMap<TypeId, SubApp>,
}

impl SubApps {
    pub fn finish_plugins(&mut self) {
        self.main.finish_plugins();
        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.finish_plugins();
        }
    }

    pub fn init(&mut self) {
        self.main.init();
        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.init();
        }
    }

    pub fn update(&mut self) {
        self.main.update();
        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.extract_from(&mut self.main.world).unwrap();
            sub_app.update();
            sub_app.write_world().increment_change_tick();
        }
        self.main.write_world().increment_change_tick();
    }

    pub fn shutdown(&mut self) {
        self.main.shutdown();
        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.shutdown();
        }
    }
}

pub struct App {
    plugins: SharedLock<Vec<Box<dyn Plugin>>>,
    runner: Option<Box<dyn Runner>>,
    sub_apps: SubApps,
}

impl App {
    pub fn empty() -> Self {
        Self {
            runner: None,
            plugins: SharedLock::new(Vec::default()),
            sub_apps: SubApps {
                main: SubApp::new(),
                sub_apps: FxHashMap::default(),
            },
        }
    }

    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let mut this = Self::empty();
        this.insert_resource(TypeRegistry::new());
        this.main_app_mut().push_init_stage::<Init>();
        this.main_app_mut().push_update_stage::<PrepareFrame>();
        this.main_app_mut().push_update_stage::<PreUpdate>();
        this.main_app_mut().push_update_stage::<Update>();
        this.main_app_mut().push_update_stage::<PostUpdate>();
        this.main_app_mut().push_update_stage::<FinishFrame>();
        this.main_app_mut().push_shutdown_stage::<Shutdown>();

        this
    }

    pub fn main_app(&self) -> &SubApp {
        &self.sub_apps.main
    }

    pub fn main_app_mut(&mut self) -> &mut SubApp {
        &mut self.sub_apps.main
    }

    pub fn add_plugin<T: Plugin>(&mut self, plugin: T) -> Result<&mut Self> {
        if self
            .plugins
            .read_arc()
            .iter()
            .any(|plugin| (**plugin).type_id() == TypeId::of::<T>())
        {
            log::warn!("Plugin already added: {:?}", plugin.name());
            return Ok(self);
        }

        log::debug!("Adding plugin: {:?}", plugin.name());
        plugin.build(self)?;

        self.plugins.write_arc().push(Box::new(plugin));
        Ok(self)
    }

    pub fn set_runner<T: Runner>(&mut self, runner: T) {
        self.runner = Some(Box::new(runner));
    }

    pub fn add_sub_app<T: AppLabel>(&mut self, app: SubApp) {
        self.sub_apps.sub_apps.insert(TypeId::of::<T>(), app);
    }

    pub fn get_sub_app<T: AppLabel>(&self) -> Option<&SubApp> {
        self.sub_apps.sub_apps.get(&TypeId::of::<T>())
    }

    pub fn get_sub_app_mut<T: AppLabel>(&mut self) -> Option<&mut SubApp> {
        self.sub_apps.sub_apps.get_mut(&TypeId::of::<T>())
    }

    pub fn remove_sub_app<T: AppLabel>(&mut self) -> Option<SubApp> {
        self.sub_apps.sub_apps.remove(&TypeId::of::<T>())
    }

    pub fn has_resource<T: Resource>(&self) -> bool {
        self.main_app().has_resource::<T>()
    }

    pub fn insert_resource<T: Resource>(&mut self, resource: T) -> &mut Self {
        self.main_app_mut().insert_resource(resource);
        self
    }

    pub fn register_type<T: Typed>(&mut self) -> &mut Self {
        self.main_app_mut()
            .get_resource_mut::<TypeRegistry>()
            .unwrap()
            .register::<T>();
        self
    }

    pub fn add_event<T: Event>(&mut self) -> &mut Self {
        fn clear_events<T: Event>(events: Res<Events<T>>, world_ticks: WorldTicks) -> Result<()> {
            events.update(world_ticks.change_tick);
            Ok(())
        }
        self.insert_resource(Events::<T>::new());
        self.add_system(clear_events::<T>, FinishFrame);
        self
    }

    pub fn add_manually_updated_event<T: Event>(&mut self) -> &mut Self {
        self.insert_resource(ManuallyUpdatedEvents::<T>::new(Events::<T>::new()));
        self
    }

    pub fn add_system<S: SystemStage, M>(
        &mut self,
        system: impl IntoSystem<M> + 'static,
        stage: S,
    ) -> &mut Self {
        self.main_app_mut().add_system(system, stage);
        self
    }

    pub fn add_system_before<S: SystemStage, M1, M2>(
        &mut self,
        system: impl IntoSystem<M1> + 'static,
        before: impl IntoSystem<M2> + 'static,
        stage: S,
    ) -> &mut Self {
        self.main_app_mut().add_system_before(system, before, stage);
        self
    }

    pub fn add_system_after<S: SystemStage, M1, M2>(
        &mut self,
        system: impl IntoSystem<M1> + 'static,
        after: impl IntoSystem<M2> + 'static,
        stage: S,
    ) -> &mut Self {
        self.main_app_mut().add_system_after(system, after, stage);
        self
    }

    pub fn init(&mut self) {
        self.sub_apps.init();
    }

    pub fn update(&mut self) {
        self.sub_apps.update();
    }

    pub fn shutdown(&mut self) {
        self.sub_apps.shutdown();
    }

    pub fn run(&mut self) -> Result<()> {
        if let Some(runner) = self.runner.take() {
            for plugin in self.plugins.read_arc().iter() {
                log::debug!("Finishing plugin: {:?}", plugin.name());
                plugin.finish(self)?;
            }

            self.sub_apps.finish_plugins();

            let result = runner.run(self);

            self.runner = Some(runner);
            result
        } else {
            Ok(())
        }
    }
}
