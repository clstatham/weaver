use std::any::{Any, TypeId};

use plugin::Plugin;

use weaver_ecs::{
    component::Res,
    system::{IntoSystem, System},
    system_schedule::SystemStage,
    world::{ConstructFromWorld, World, WorldTicks},
};
use weaver_event::{Event, Events, ManuallyUpdatedEvents};
use weaver_util::{maps::TypeIdSet, FxHashMap, Result};

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
    T: Fn(&mut App) -> Result<()> + 'static,
{
    fn run(&self, app: &mut App) -> Result<()> {
        self(app)
    }
}

pub type ExtractFn = Box<dyn Fn(&mut World, &mut World) -> Result<()> + Send + Sync>;

pub trait AppLabel: 'static {}

pub struct SubApp {
    world: World,
    plugins: Vec<(TypeId, Box<dyn Plugin>)>,
    unready_plugins: TypeIdSet,
    extract_fn: Option<ExtractFn>,
}

impl Default for SubApp {
    fn default() -> Self {
        let world = World::new();

        Self {
            world,
            plugins: Vec::new(),
            unready_plugins: TypeIdSet::default(),
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
        let plugins = std::mem::take(&mut self.plugins);
        let unready_plugins = self.unready_plugins.clone();
        for type_id in unready_plugins.iter() {
            let (_, plugin) = plugins.iter().find(|(id, _)| *id == *type_id).unwrap();
            let ready = self
                .as_app(|app| -> Result<bool> {
                    if !plugin.ready(app) {
                        log::debug!("Plugin is not ready: {:?}", plugin.name());
                        return Ok(false);
                    }
                    log::debug!("Finishing plugin: {:?}", plugin.name());
                    plugin.finish(app)?;
                    Ok(true)
                })
                .unwrap();
            if ready {
                self.unready_plugins.remove(type_id);
            }
        }
        self.plugins = plugins;
    }

    pub fn add_plugin<T: Plugin>(&mut self, plugin: T) -> Result<&mut Self> {
        if self
            .plugins
            .iter()
            .any(|(_, plugin)| (**plugin).type_id() == TypeId::of::<T>())
        {
            log::warn!("Plugin already added: {:?}", plugin.name());
            return Ok(self);
        }

        log::debug!("Adding plugin: {:?}", plugin.name());
        self.as_app(|app| plugin.build(app))?;

        self.plugins.push((TypeId::of::<T>(), Box::new(plugin)));
        self.unready_plugins.insert(TypeId::of::<T>());

        Ok(self)
    }

    pub fn set_extract(&mut self, extract: ExtractFn) -> &mut Self {
        self.extract_fn = Some(extract);
        self
    }

    pub fn extract_fn(&self) -> Option<&ExtractFn> {
        self.extract_fn.as_ref()
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn extract_from(&mut self, from: &mut World) -> Result<()> {
        if let Some(extract_fn) = self.extract_fn.as_mut() {
            extract_fn(from, &mut self.world)
        } else {
            Ok(())
        }
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

    pub async fn init(&mut self) {
        self.main.world_mut().init().await.unwrap();
        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.world_mut().init().await.unwrap();
        }
    }

    pub async fn update(&mut self) {
        self.main.world_mut().update().await.unwrap();
        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.extract_from(&mut self.main.world).unwrap();
            sub_app.world_mut().update().await.unwrap();
            sub_app.world_mut().increment_change_tick();
        }
        self.main.world_mut().increment_change_tick();
    }

    pub async fn shutdown(&mut self) {
        self.main.world_mut().shutdown().await.unwrap();
        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.world_mut().shutdown().await.unwrap();
        }
    }
}

pub struct App {
    plugins: Vec<(TypeId, Box<dyn Plugin>)>,
    unready_plugins: TypeIdSet,
    runner: Option<Box<dyn Runner>>,
    sub_apps: SubApps,
}

impl App {
    pub fn empty() -> Self {
        Self {
            runner: None,
            plugins: Vec::new(),
            unready_plugins: TypeIdSet::default(),
            sub_apps: SubApps {
                main: SubApp::new(),
                sub_apps: FxHashMap::default(),
            },
        }
    }

    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let mut this = Self::empty();
        this.main_app_mut().world_mut().push_init_stage::<Init>();
        this.main_app_mut()
            .world_mut()
            .push_update_stage::<PrepareFrame>();
        this.main_app_mut()
            .world_mut()
            .push_update_stage::<PreUpdate>();
        this.main_app_mut()
            .world_mut()
            .push_update_stage::<Update>();
        this.main_app_mut()
            .world_mut()
            .push_update_stage::<PostUpdate>();
        this.main_app_mut()
            .world_mut()
            .push_update_stage::<FinishFrame>();
        this.main_app_mut()
            .world_mut()
            .push_shutdown_stage::<Shutdown>();

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
            .iter()
            .any(|(_, plugin)| (**plugin).type_id() == TypeId::of::<T>())
        {
            log::warn!("Plugin already added: {:?}", plugin.name());
            return Ok(self);
        }

        log::debug!("Adding plugin: {:?}", plugin.name());
        plugin.build(self)?;

        self.plugins.push((TypeId::of::<T>(), Box::new(plugin)));
        self.unready_plugins.insert(TypeId::of::<T>());
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

    pub fn configure_sub_app<T: AppLabel>(&mut self, f: impl FnOnce(&mut SubApp)) -> &mut Self {
        if let Some(sub_app) = self.get_sub_app_mut::<T>() {
            f(sub_app);
        }
        self
    }

    pub fn remove_sub_app<T: AppLabel>(&mut self) -> Option<SubApp> {
        self.sub_apps.sub_apps.remove(&TypeId::of::<T>())
    }

    pub fn has_resource<T: Any + Send + Sync>(&self) -> bool {
        self.main_app().world().has_resource::<T>()
    }

    pub fn init_resource<T: Any + Send + Sync + ConstructFromWorld>(&mut self) -> &mut Self {
        self.main_app_mut().world_mut().init_resource::<T>();
        self
    }

    pub fn insert_resource<T: Any + Send + Sync>(&mut self, resource: T) -> &mut Self {
        self.main_app().world().insert_resource(resource);
        self
    }

    pub fn add_event<T: Event>(&mut self) -> &mut Self {
        async fn clear_events<T: Event>(events: Res<Events<T>>, world_ticks: WorldTicks) {
            events.update(world_ticks.change_tick);
        }
        self.insert_resource(Events::<T>::new());
        self.main_app_mut()
            .world_mut()
            .add_system(clear_events::<T>, FinishFrame);
        self
    }

    pub fn add_manually_updated_event<T: Event>(&mut self) -> &mut Self {
        self.insert_resource(ManuallyUpdatedEvents::<T>::new(Events::<T>::new()));
        self
    }

    pub fn send_event<T: Event>(&self, event: T) {
        self.main_app()
            .world()
            .get_resource::<Events<T>>()
            .unwrap()
            .send(event);
    }

    pub fn add_system<T, M, S>(&mut self, system: S, stage: T) -> &mut Self
    where
        T: SystemStage,
        M: 'static,
        S: IntoSystem<M>,
        S::System: System,
    {
        self.main_app_mut().world_mut().add_system(system, stage);
        self
    }

    pub fn add_system_before<T, M1, M2, S, BEFORE>(
        &mut self,
        system: S,
        before: BEFORE,
        stage: T,
    ) -> &mut Self
    where
        T: SystemStage,
        M1: 'static,
        M2: 'static,
        S: IntoSystem<M1>,
        BEFORE: IntoSystem<M2>,
        S::System: System,
        BEFORE::System: System,
    {
        self.main_app_mut()
            .world_mut()
            .add_system_before(system, before, stage);
        self
    }

    pub fn add_system_after<T, M1, M2, S, AFTER>(
        &mut self,
        system: S,
        after: AFTER,
        stage: T,
    ) -> &mut Self
    where
        T: SystemStage,
        M1: 'static,
        M2: 'static,
        S: IntoSystem<M1>,
        AFTER: IntoSystem<M2>,
        S::System: System,
        AFTER::System: System,
    {
        self.main_app_mut()
            .world_mut()
            .add_system_after(system, after, stage);
        self
    }

    pub async fn init(&mut self) {
        self.finish_plugins();
        self.sub_apps.init().await;
    }

    pub async fn update(&mut self) {
        while !self.unready_plugins.is_empty() {
            self.finish_plugins();
        }

        self.sub_apps.update().await;
    }

    pub async fn shutdown(&mut self) {
        self.sub_apps.shutdown().await;
    }

    pub fn finish_plugins(&mut self) {
        let plugins = std::mem::take(&mut self.plugins);
        let unready_plugins = self.unready_plugins.clone();
        for type_id in unready_plugins.iter() {
            let (_, plugin) = plugins.iter().find(|(id, _)| *id == *type_id).unwrap();
            let ready = plugin.ready(self);
            if ready {
                log::debug!("Finishing plugin: {:?}", plugin.name());
                plugin.finish(self).unwrap();
                self.unready_plugins.remove(type_id);
            }
        }
        self.plugins = plugins;
    }

    pub fn run(&mut self) -> Result<()> {
        if let Some(runner) = self.runner.take() {
            self.finish_plugins();

            let result = runner.run(self);

            self.runner = Some(runner);
            result
        } else {
            Ok(())
        }
    }
}
