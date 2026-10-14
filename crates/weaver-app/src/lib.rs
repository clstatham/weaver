//! The application container: worlds, plugins and the main loop.
//!
//! An [`App`] owns a main [`SubApp`] (a [`World`] plus its own plugin list) and any number of named
//! sub-apps keyed by an [`AppLabel`] type. The renderer, for instance, keeps its own world in a
//! sub-app and pulls data out of the main world each frame through the sub-app's extract function
//! ([`SubApp::set_extract`]). Resource, system and event helpers on [`App`] always target the main
//! sub-app's world.
//!
//! # Plugins
//!
//! Features are added as [`Plugin`]s. The lifecycle is `build`, then `ready`, then `finish`
//! (and `cleanup`, only when a plugin is reconfigured with [`App::configure_plugin`]).
//! `build` runs immediately inside [`App::add_plugin`] and is where a plugin registers its
//! resources, systems and events. `finish` is deferred until the plugin's `ready` returns true;
//! plugins that need something that only exists later (a window, say) return false until it does.
//! [`App::finish_plugins`] is polled by the runner, and [`App::update`] won't run a frame until
//! every App-level plugin has finished.
//!
//! # Stages
//!
//! [`App::new`] registers the [`AppStage`]s on the main world: `Init` runs once at startup,
//! `PrepareFrame`, `PreUpdate`, `Update`, `PostUpdate` and `FinishFrame` run in that order every
//! frame, and `Shutdown` runs once on exit. Other crates push their own stages after those, so
//! they run after `FinishFrame`. A bare [`SubApp`] has no stages at all.
//!
//! # Running
//!
//! [`App::run`] hands the app to the [`Runner`] set with [`App::set_runner`] (the winit plugin
//! installs one) and the runner calls [`App::init`], then [`App::update`] once per frame, and
//! finally [`App::shutdown`]. With no runner, `run` does nothing.
//!
//! [`World`]: weaver_ecs::world::World

use std::any::TypeId;

use plugin::{DummyPlugin, Plugin};

use weaver_ecs::{
    SystemStage,
    prelude::{Component, ResMut},
    system::{IntoSystem, IntoSystemConfig},
    system_schedule::SystemStage,
    world::{ConstructFromWorld, World},
};
use weaver_event::{Event, EventQueue};
use weaver_util::prelude::*;

pub mod plugin;

/// The names most apps want in scope, including the bare [`AppStage`] variants.
pub mod prelude {
    pub use crate::{App, AppStage, AppStage::*, SubApp, plugin::Plugin};
}

/// The stages [`App::new`] registers on the main world.
///
/// `Init` and `Shutdown` run once; the five in between make up one frame, in declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
pub enum AppStage {
    /// Runs once at startup, from [`App::init`].
    Init,
    /// First stage of every frame; per-frame setup that the rest of the frame relies on.
    PrepareFrame,
    /// Runs before the main game logic (input handling, timers).
    PreUpdate,
    /// Main game logic.
    Update,
    /// Runs after [`AppStage::Update`], for systems that react to what it did.
    PostUpdate,
    /// Last built-in stage of a frame. Event queues added with [`App::add_event`] are cleared here.
    FinishFrame,
    /// Runs once on exit, from [`App::shutdown`].
    Shutdown,
}

/// Owns the main loop of an [`App`].
///
/// Install one with [`App::set_runner`]; [`App::run`] then calls it. Any
/// `Fn(&mut App) -> Result<()>` closure is a runner. A runner is responsible for calling
/// [`App::init`], [`App::update`] each frame and [`App::shutdown`].
pub trait Runner: 'static {
    /// Runs the app until it exits.
    ///
    /// # Errors
    ///
    /// Returns whatever error the runner hit; it is passed straight out of [`App::run`].
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

/// Copies data from the main world (first argument) into a sub-app's world (second argument).
///
/// Called once per frame, right before the sub-app's world updates. See [`SubApp::set_extract`].
pub type ExtractFn = Box<dyn Fn(&mut World, &mut World) -> Result<()> + Send + Sync>;

/// Marker type that names a sub-app. Sub-apps are keyed by the [`TypeId`] of the label type.
pub trait AppLabel: 'static {}

/// A [`World`] with its own plugin list and an optional extract function.
///
/// The main world of an [`App`] is a `SubApp`, and so is each named sub-app (the render world, for
/// example). A fresh `SubApp` has no stages registered; push them on [`SubApp::world_mut`] before
/// adding plugins that use them.
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
    /// Creates a sub-app with an empty world, no plugins and no extract function.
    pub fn new() -> Self {
        Self::default()
    }

    // Runs `f` with this sub-app temporarily installed as the main sub-app of a throwaway `App`,
    // so `Plugin` methods (which take `&mut App`) can be called against it.
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

    /// Calls `ready` on every plugin that hasn't finished yet and `finish` on those that are ready.
    ///
    /// Plugins that aren't ready are tried again on the next call.
    ///
    /// # Panics
    ///
    /// Panics if a plugin's `finish` returns an error.
    pub fn finish_plugins(&mut self) {
        // FIXME: plugin added in `finish` is lost on restore; next pass panics on find().unwrap()
        let plugins = std::mem::take(&mut self.plugins);
        let unready_plugins = self.unready_plugins.clone();
        for type_id in unready_plugins.iter() {
            let (_, plugin) = plugins.iter().find(|(id, _)| *id == *type_id).unwrap();
            let ready = self
                // FIXME: throwaway App, so nested add_plugin here never gets ready/finish
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

    /// Builds `plugin` against this sub-app and records it for `finish_plugins`.
    ///
    /// Adding a plugin type that is already present logs a warning and does nothing.
    ///
    /// # Errors
    ///
    /// Returns the error from the plugin's `build`.
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

    /// Sets the function that pulls data from the main world into this sub-app's world.
    pub fn set_extract(&mut self, extract: ExtractFn) -> &mut Self {
        self.extract_fn = Some(extract);
        self
    }

    /// Returns the extract function, if one was set.
    pub fn extract_fn(&self) -> Option<&ExtractFn> {
        self.extract_fn.as_ref()
    }

    /// Returns this sub-app's world.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Returns this sub-app's world mutably.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Runs the extract function with `from` as the source and this sub-app's world as the target.
    ///
    /// Does nothing if no extract function was set.
    ///
    /// # Errors
    ///
    /// Returns the error from the extract function.
    pub fn extract_from(&mut self, from: &mut World) -> Result<()> {
        if let Some(extract_fn) = self.extract_fn.as_mut() {
            extract_fn(from, &mut self.world)
        } else {
            Ok(())
        }
    }

    /// Inserts a fresh [`EventQueue<T>`] into this sub-app's world and clears it in
    /// `clear_events_stage`.
    ///
    /// Unlike [`App::add_event`] this doesn't check for an existing queue; calling it twice for the
    /// same event type replaces the queue.
    pub fn add_event<T: Event>(&mut self, clear_events_stage: impl SystemStage) -> &mut Self {
        fn clear_events<T: Event>(mut events: ResMut<EventQueue<T>>) {
            events.update();
        }
        self.world_mut().insert_resource(EventQueue::<T>::new());
        self.world_mut()
            .add_system(clear_events::<T>, clear_events_stage);
        self
    }
}

/// The main [`SubApp`] plus the named ones, updated together.
///
/// Each operation runs on the main sub-app first and then on every named sub-app, in no
/// particular order. All of them panic if a world operation fails.
pub struct SubApps {
    /// The main sub-app, where user systems and resources live.
    pub main: SubApp,
    sub_apps: FxHashMap<TypeId, SubApp>,
}

impl SubApps {
    /// Runs [`SubApp::finish_plugins`] on every sub-app.
    pub fn finish_plugins(&mut self) {
        self.main.finish_plugins();
        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.finish_plugins();
        }
    }

    /// Initializes systems and runs the init stages of every world.
    ///
    /// # Panics
    ///
    /// Panics if running a world's init stages fails.
    pub fn init(&mut self) {
        self.main.world_mut().initialize_systems();
        self.main.world_mut().init().unwrap();
        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.world_mut().initialize_systems();
            sub_app.world_mut().init().unwrap();
        }
    }

    /// Updates the main world, then extracts into and updates each named sub-app.
    ///
    /// # Panics
    ///
    /// Panics if a world update or an extract function fails.
    pub fn update(&mut self) {
        self.main.world_mut().update().unwrap();
        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.extract_from(&mut self.main.world).unwrap();
            sub_app.world_mut().update().unwrap();
        }
    }

    /// Runs the shutdown stages of every world.
    ///
    /// # Panics
    ///
    /// Panics if a world's shutdown stages fail.
    pub fn shutdown(&mut self) {
        self.main.world_mut().shutdown().unwrap();
        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.world_mut().shutdown().unwrap();
        }
    }
}

/// The application: a main [`SubApp`], named sub-apps, App-level plugins and a [`Runner`].
///
/// App-level plugins (added with [`App::add_plugin`]) and the main sub-app's own plugin list are
/// separate; the former are built against the whole `App`, so they can add sub-apps and set the
/// runner. See the [crate docs](crate) for the overall flow.
pub struct App {
    plugins: Vec<(TypeId, Box<dyn Plugin>)>,
    unready_plugins: TypeIdSet,
    runner: Option<Box<dyn Runner>>,
    sub_apps: SubApps,
}

impl App {
    /// Creates an app with an empty main world: no stages, no tracing subscriber.
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

    /// Creates an app with the [`AppStage`]s registered and a tracing subscriber installed.
    ///
    /// The subscriber filters with `RUST_LOG` (via `EnvFilter`); if one is already installed, it's
    /// left alone.
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

    /// Returns the main sub-app.
    pub fn main_app(&self) -> &SubApp {
        &self.sub_apps.main
    }

    /// Returns the main sub-app mutably.
    pub fn main_app_mut(&mut self) -> &mut SubApp {
        &mut self.sub_apps.main
    }

    /// Builds `plugin` against this app and records it so `finish` runs once it is ready.
    ///
    /// `build` runs right away, so a plugin that adds other plugins from `build` has them built
    /// first. Adding a plugin type that is already present logs a warning and does nothing.
    ///
    /// # Errors
    ///
    /// Returns the error from the plugin's `build`.
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

    /// Tweaks an already-added plugin by running its `cleanup`, applying `f`, and rebuilding it.
    ///
    /// Does nothing if no plugin of type `T` was added. The plugin's `finish` isn't run again.
    ///
    /// # Panics
    ///
    /// Panics if the plugin's `cleanup` or `build` returns an error.
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

    /// Sets the [`Runner`] that [`App::run`] will hand control to, replacing any previous one.
    pub fn set_runner<T: Runner>(&mut self, runner: T) {
        self.runner = Some(Box::new(runner));
    }

    /// Registers `app` as the sub-app named by label `T`, replacing any sub-app already there.
    pub fn add_sub_app<T: AppLabel>(&mut self, app: SubApp) {
        self.sub_apps.sub_apps.insert(TypeId::of::<T>(), app);
    }

    /// Returns the sub-app named by label `T`, if there is one.
    pub fn get_sub_app<T: AppLabel>(&self) -> Option<&SubApp> {
        self.sub_apps.sub_apps.get(&TypeId::of::<T>())
    }

    /// Returns the sub-app named by label `T` mutably, if there is one.
    pub fn get_sub_app_mut<T: AppLabel>(&mut self) -> Option<&mut SubApp> {
        self.sub_apps.sub_apps.get_mut(&TypeId::of::<T>())
    }

    /// Runs `f` on the sub-app named by label `T`; does nothing if there isn't one.
    pub fn configure_sub_app<T: AppLabel>(&mut self, f: impl FnOnce(&mut SubApp)) -> &mut Self {
        if let Some(sub_app) = self.get_sub_app_mut::<T>() {
            f(sub_app);
        }
        self
    }

    /// Removes and returns the sub-app named by label `T`.
    pub fn remove_sub_app<T: AppLabel>(&mut self) -> Option<SubApp> {
        self.sub_apps.sub_apps.remove(&TypeId::of::<T>())
    }

    /// Returns whether the main world has a resource of type `T`.
    pub fn has_resource<T: Component>(&self) -> bool {
        self.main_app().world().has_resource::<T>()
    }

    /// Initializes a resource in the main world from its [`ConstructFromWorld`] impl.
    pub fn init_resource<T: Component + ConstructFromWorld>(&mut self) -> &mut Self {
        self.main_app_mut().world_mut().init_resource::<T>();
        self
    }

    /// Inserts `resource` into the main world, replacing any existing one of the same type.
    pub fn insert_resource<T: Component>(&mut self, resource: T) -> &mut Self {
        self.main_app().world().insert_resource(resource);
        self
    }

    /// Registers event type `T` on the main world.
    ///
    /// Inserts the [`EventQueue<T>`] and a system that swaps its buffers in
    /// [`AppStage::FinishFrame`]. Does nothing if the queue already exists.
    pub fn add_event<T: Event>(&mut self) -> &mut Self {
        if self.has_resource::<EventQueue<T>>() {
            return self;
        }
        self.insert_resource(EventQueue::<T>::new());
        self.main_app_mut()
            .world_mut()
            .add_system(clear_events::<T>, AppStage::FinishFrame);
        self
    }

    /// Removes the event queue for `T` and its clearing system from the main world.
    pub fn remove_event<T: Event>(&mut self) -> &mut Self {
        let world = self.main_app_mut().world_mut();
        world.remove_resource::<EventQueue<T>>();
        world.remove_system(clear_events::<T>, AppStage::FinishFrame);
        self
    }

    /// Inserts an [`EventQueue<T>`] with no clearing system.
    ///
    /// The caller has to call [`EventQueue::update`] or [`EventQueue::clear`] itself, otherwise the
    /// queue grows forever.
    pub fn add_manually_updated_event<T: Event>(&mut self) -> &mut Self {
        self.insert_resource(EventQueue::<T>::new());
        self
    }

    /// Sends `event` on the main world's queue for `T`.
    ///
    /// # Panics
    ///
    /// Panics if the event type was never registered (see [`App::add_event`]).
    pub fn send_event<T: Event>(&self, event: T) {
        self.main_app()
            .world()
            .get_resource_mut::<EventQueue<T>>()
            .unwrap()
            .send(event);
    }

    /// Adds `system` to `stage` in the main world.
    pub fn add_system<T, M, S>(&mut self, system: S, stage: T) -> &mut Self
    where
        T: SystemStage,
        M: 'static,
        S: IntoSystemConfig<M>,
    {
        self.main_app_mut().world_mut().add_system(system, stage);
        self
    }

    /// Makes `run_first` run before `run_second` within `stage` in the main world.
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

    /// Finishes whatever plugins are ready, then initializes systems and runs the init stages.
    ///
    /// Called by the runner before the first frame.
    ///
    /// # Panics
    ///
    /// Panics if a plugin's `finish` or an init stage fails.
    pub fn init(&mut self) {
        self.finish_plugins();
        self.sub_apps.init();
    }

    /// Runs one frame: every world's update stages, with extraction into each named sub-app.
    ///
    /// Waits for all App-level plugins to be finished before running anything.
    ///
    /// # Panics
    ///
    /// Panics if a plugin's `finish`, a world update or an extract function fails.
    pub fn update(&mut self) {
        // FIXME: busy-loops until every plugin is ready; hangs if one never is
        while !self.unready_plugins.is_empty() {
            self.finish_plugins();
        }

        self.sub_apps.update();
    }

    /// Runs the shutdown stages of every world.
    ///
    /// # Panics
    ///
    /// Panics if a shutdown stage fails.
    pub fn shutdown(&mut self) {
        self.sub_apps.shutdown();
    }

    /// Calls `ready` on every unfinished plugin and `finish` on those that are ready, for this app
    /// and then for all sub-apps.
    ///
    /// Plugins that aren't ready are tried again on the next call.
    ///
    /// # Panics
    ///
    /// Panics if a plugin's `finish` returns an error.
    pub fn finish_plugins(&mut self) {
        // FIXME: plugin added in `finish` is lost on restore; next pass panics on find().unwrap()
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

    /// Finishes the ready plugins and hands the app to the runner.
    ///
    /// Returns `Ok(())` without doing anything if no runner was set.
    ///
    /// # Errors
    ///
    /// Returns the runner's error.
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

fn clear_events<T: Event>(mut events: ResMut<EventQueue<T>>) {
    events.update();
}
