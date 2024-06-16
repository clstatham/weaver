use std::{
    any::TypeId,
    ops::{Deref, DerefMut},
};

use plugin::Plugin;
use rustc_hash::FxHashMap;
use system::{FunctionSystem, SystemGraph, SystemStage};
use weaver_ecs::{
    component::{Res, ResMut, Resource},
    reflect::registry::{TypeRegistry, Typed},
    world::World,
};
use weaver_event::{Event, Events};
use weaver_util::{lock::SharedLock, prelude::Result};

pub mod commands;
pub mod plugin;
pub mod system;

pub mod prelude {
    pub use crate::plugin::Plugin;
    pub use crate::App;
}

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

pub type ExtractFn = Box<dyn Fn(&mut World, &mut World) -> Result<()>>;

pub trait AppLabel: 'static {}

pub struct SubApp {
    world: World,
    systems: SharedLock<FxHashMap<SystemStage, SystemGraph>>,
    plugins: SharedLock<Vec<Box<dyn Plugin>>>,
    extract_fn: Option<ExtractFn>,
}

impl Default for SubApp {
    fn default() -> Self {
        let world = World::new();

        Self {
            world,
            systems: SharedLock::new(FxHashMap::default()),
            plugins: SharedLock::new(Vec::default()),
            extract_fn: None,
        }
    }
}

impl SubApp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn as_app<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut App) -> R,
    {
        let mut app = App::empty();
        std::mem::swap(&mut app.sub_apps.main, self);
        let result = f(&mut app);
        std::mem::swap(&mut app.sub_apps.main, self);
        result
    }

    pub fn insert_resource<T: Resource>(&self, resource: T) -> &Self {
        self.world.insert_resource(resource);
        self
    }

    pub fn get_resource<T: Resource>(&self) -> Option<Res<T>> {
        self.world.get_resource::<T>()
    }

    pub fn get_resource_mut<T: Resource>(&self) -> Option<ResMut<T>> {
        self.world.get_resource_mut::<T>()
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

        let name = plugin.name().to_owned();
        log::debug!("Adding plugin: {:?}", &name);
        self.as_app(|app| plugin.build(app))?;

        self.plugins.write_arc().push(Box::new(plugin));

        Ok(self)
    }

    pub fn add_system<M>(
        &mut self,
        system: impl FunctionSystem<M> + 'static,
        stage: SystemStage,
    ) -> &mut Self {
        self.systems
            .write_arc()
            .entry(stage)
            .or_default()
            .add_system(system);
        self
    }

    pub fn add_system_before<M1, M2>(
        &mut self,
        system: impl FunctionSystem<M1> + 'static,
        before: impl FunctionSystem<M2> + 'static,
        stage: SystemStage,
    ) -> &mut Self {
        self.systems
            .write_arc()
            .entry(stage)
            .or_default()
            .add_system_before(system, before);
        self
    }

    pub fn add_system_after<M1, M2>(
        &mut self,
        system: impl FunctionSystem<M1> + 'static,
        after: impl FunctionSystem<M2> + 'static,
        stage: SystemStage,
    ) -> &mut Self {
        self.systems
            .write_arc()
            .entry(stage)
            .or_default()
            .add_system_after(system, after);
        self
    }

    pub fn run_systems(&mut self, stage: SystemStage) -> Result<()> {
        let systems = self.systems.read_arc();
        if let Some(systems) = systems.get(&stage) {
            systems.run(&mut self.world)?;
        }
        Ok(())
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

pub struct App {
    runner: Option<Box<dyn Runner>>,
    sub_apps: SubApps,
}

impl Deref for App {
    type Target = SubApp;

    fn deref(&self) -> &Self::Target {
        &self.sub_apps.main
    }
}

impl DerefMut for App {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sub_apps.main
    }
}

impl App {
    pub fn empty() -> Self {
        Self {
            runner: None,
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
            .main_app()
            .plugins
            .read_arc()
            .iter()
            .any(|plugin| (**plugin).type_id() == TypeId::of::<T>())
        {
            log::warn!("Plugin already added: {:?}", plugin.name());
            return Ok(self);
        }

        let name = plugin.name().to_owned();
        log::debug!("Adding plugin: {:?}", &name);
        plugin.build(self)?;

        self.main_app().plugins.write_arc().push(Box::new(plugin));

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

    pub fn insert_resource<T: Resource>(&mut self, resource: T) -> &mut Self {
        self.main_app().insert_resource(resource);
        self
    }

    pub fn register_type<T: Typed>(&mut self) -> &mut Self {
        self.get_resource_mut::<TypeRegistry>()
            .unwrap()
            .register::<T>();
        self
    }

    pub fn add_event<T: Event>(&mut self) -> &mut Self {
        fn update_events<T: Event>(mut events: ResMut<Events<T>>) -> Result<()> {
            events.clear();
            Ok(())
        }
        self.insert_resource(Events::<T>::new());
        self.add_system(update_events::<T>, SystemStage::PrepareFrame);
        self
    }

    pub fn add_system<M>(
        &mut self,
        system: impl FunctionSystem<M> + 'static,
        stage: SystemStage,
    ) -> &mut Self {
        self.main_app_mut().add_system(system, stage);
        self
    }

    pub fn add_system_before<M1, M2>(
        &mut self,
        system: impl FunctionSystem<M1> + 'static,
        before: impl FunctionSystem<M2> + 'static,
        stage: SystemStage,
    ) -> &mut Self {
        self.main_app_mut().add_system_before(system, before, stage);
        self
    }

    pub fn add_system_after<M1, M2>(
        &mut self,
        system: impl FunctionSystem<M1> + 'static,
        after: impl FunctionSystem<M2> + 'static,
        stage: SystemStage,
    ) -> &mut Self {
        self.main_app_mut().add_system_after(system, after, stage);
        self
    }

    pub fn run(&mut self) -> Result<()> {
        for plugin in self.plugins.read_arc().iter() {
            plugin.finish(self)?;
        }
        // todo: prevent infinite loop here
        while let Some(plugin) = self
            .plugins
            .read_arc()
            .iter()
            .find(|plugin| !plugin.ready(self))
        {
            plugin.finish(self)?;
        }

        if let Some(runner) = self.runner.take() {
            let result = runner.run(self);
            self.runner = Some(runner);
            result
        } else {
            Ok(())
        }
    }
}
