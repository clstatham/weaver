use std::any::TypeId;

use plugin::{DummyPlugin, Plugin};

use weaver_ecs::{
    SystemStage,
    change_detection::WorldTicks,
    prelude::{Component, ResMut},
    system::{IntoSystem, IntoSystemConfig},
    system_schedule::SystemStage,
    world::{ConstructFromWorld, World},
};
use weaver_event::{Event, Events};
use weaver_task::{
    task_pool::TaskPool,
    usages::{GlobalTaskPool, tick_task_pools},
};
use weaver_util::prelude::*;

pub mod plugin;

pub mod prelude {
    pub use crate::{App, AppStage, AppStage::*, SubApp, plugin::Plugin};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
pub enum AppStage {
    Init,
    PrepareFrame,
    PreUpdate,
    Update,
    PostUpdate,
    FinishFrame,
    Shutdown,
}

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
                        log::debug!("Plugin is not ready: {:?}", plugin.type_name());
                        return Ok(false);
                    }
                    log::debug!("Finishing plugin: {:?}", plugin.type_name());
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
            log::warn!("Plugin already added: {:?}", plugin.type_name());
            return Ok(self);
        }

        log::debug!("Adding plugin: {:?}", plugin.type_name());
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

    pub fn add_event<T: Event>(&mut self, clear_events_stage: impl SystemStage) -> &mut Self {
        async fn clear_events<T: Event>(mut events: ResMut<Events<T>>, world_ticks: WorldTicks) {
            events.update(world_ticks.change_tick).await;
        }
        self.world_mut().insert_resource(Events::<T>::new());
        self.world_mut()
            .add_system(clear_events::<T>, clear_events_stage);
        self
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
        self.main.world_mut().initialize_systems();
        self.main.world_mut().init().unwrap();
        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.world_mut().initialize_systems();
            sub_app.world_mut().init().unwrap();
        }
    }

    pub fn update(&mut self) {
        self.main.world_mut().update().unwrap();
        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.extract_from(&mut self.main.world).unwrap();
            sub_app.world_mut().update().unwrap();
        }
    }

    pub fn shutdown(&mut self) {
        self.main.world_mut().shutdown().unwrap();
        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.world_mut().shutdown().unwrap();
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
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init()
            .ok();

        let mut this = Self::empty();

        this.main_app_mut()
            .world_mut()
            .push_init_stage(AppStage::Init);

        this.main_app_mut()
            .world_mut()
            .push_update_stage(AppStage::PrepareFrame);
        this.main_app_mut()
            .world_mut()
            .push_update_stage(AppStage::PreUpdate);
        this.main_app_mut()
            .world_mut()
            .push_update_stage(AppStage::Update);
        this.main_app_mut()
            .world_mut()
            .push_update_stage(AppStage::PostUpdate);
        this.main_app_mut()
            .world_mut()
            .push_update_stage(AppStage::FinishFrame);

        this.main_app_mut()
            .world_mut()
            .push_shutdown_stage(AppStage::Shutdown);

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
            log::warn!("Plugin already added: {:?}", plugin.type_name());
            return Ok(self);
        }

        log::debug!("Adding plugin: {:?}", plugin.type_name());
        plugin.build(self)?;

        self.plugins.push((TypeId::of::<T>(), Box::new(plugin)));
        self.unready_plugins.insert(TypeId::of::<T>());
        Ok(self)
    }

    pub fn configure_plugin<T: Plugin>(&mut self, f: impl FnOnce(&mut T)) -> &mut Self {
        if let Some(index) = self
            .plugins
            .iter()
            .position(|(id, _)| *id == TypeId::of::<T>())
        {
            let (_, mut plugin) = std::mem::replace(
                &mut self.plugins[index],
                (TypeId::of::<DummyPlugin>(), Box::new(DummyPlugin)),
            );
            plugin.cleanup(self).unwrap();
            f(plugin.downcast_mut().unwrap());
            plugin.build(self).unwrap();
            self.plugins[index] = (TypeId::of::<T>(), plugin);
        }
        self
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

    pub fn has_resource<T: Component>(&self) -> bool {
        self.main_app().world().has_resource::<T>()
    }

    pub fn init_resource<T: Component + ConstructFromWorld>(&mut self) -> &mut Self {
        self.main_app_mut().world_mut().init_resource::<T>();
        self
    }

    pub fn insert_resource<T: Component>(&mut self, resource: T) -> &mut Self {
        self.main_app().world().insert_resource(resource);
        self
    }

    pub fn add_event<T: Event>(&mut self) -> &mut Self {
        async fn clear_events<T: Event>(mut events: ResMut<Events<T>>, world_ticks: WorldTicks) {
            events.update(world_ticks.change_tick).await;
        }
        self.insert_resource(Events::<T>::new());
        self.main_app_mut()
            .world_mut()
            .add_system(clear_events::<T>, AppStage::FinishFrame);
        self
    }

    pub fn add_manually_updated_event<T: Event>(&mut self) -> &mut Self {
        self.insert_resource(Events::<T>::new());
        self
    }

    pub async fn send_event<T: Event>(&self, event: T) {
        self.main_app()
            .world()
            .get_resource::<Events<T>>()
            .unwrap()
            .send(event)
            .await;
    }

    pub fn add_system<T, M, S>(&mut self, system: S, stage: T) -> &mut Self
    where
        T: SystemStage,
        M: 'static,
        S: IntoSystemConfig<M>,
    {
        self.main_app_mut().world_mut().add_system(system, stage);
        self
    }

    pub fn order_systems<M1, M2, S1, S2>(
        &mut self,
        run_first: S1,
        run_second: S2,
        stage: impl SystemStage,
    ) -> &mut Self
    where
        M1: 'static,
        M2: 'static,
        S1: IntoSystem<M1>,
        S2: IntoSystem<M2>,
    {
        self.main_app_mut()
            .world_mut()
            .order_systems(run_first, run_second, stage);
        self
    }

    pub fn init(&mut self) {
        self.finish_plugins();
        self.sub_apps.init();
    }

    pub fn update(&mut self) {
        while !self.unready_plugins.is_empty() {
            self.finish_plugins();
        }

        self.sub_apps.update();
        tick_task_pools();
    }

    pub fn shutdown(&mut self) {
        self.sub_apps.shutdown();
    }

    pub fn finish_plugins(&mut self) {
        let plugins = std::mem::take(&mut self.plugins);
        let unready_plugins = self.unready_plugins.clone();
        for type_id in unready_plugins.iter() {
            let (_, plugin) = plugins.iter().find(|(id, _)| *id == *type_id).unwrap();
            let ready = plugin.ready(self);
            if ready {
                log::debug!("Finishing plugin: {:?}", plugin.type_name());
                plugin.finish(self).unwrap();
                self.unready_plugins.remove(type_id);
            } else {
                log::debug!("Plugin is not ready: {:?}", plugin.type_name());
            }
        }
        self.plugins = plugins;

        self.sub_apps.finish_plugins();
    }

    pub fn run(&mut self) -> Result<()> {
        GlobalTaskPool::get_or_init(TaskPool::new);

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
