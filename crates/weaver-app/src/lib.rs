use std::{rc::Rc, sync::Arc};

use plugin::Plugin;
use rustc_hash::FxHashMap;
use weaver_ecs::{
    component::Component,
    scene::Scene,
    storage::{Mut, Ref},
    system::{System, SystemStage},
    world::World,
};
use weaver_util::lock::SharedLock;

pub mod plugin;

pub mod prelude {
    pub use crate::plugin::Plugin;
    pub use crate::App;
}

pub trait Runner: 'static {
    fn run(&self, app: App) -> anyhow::Result<()>;
}

impl<T> Runner for T
where
    T: Fn(App) -> anyhow::Result<()> + Send + Sync + 'static,
{
    fn run(&self, app: App) -> anyhow::Result<()> {
        self(app)
    }
}

pub struct App {
    world: Rc<World>,
    systems: SharedLock<FxHashMap<SystemStage, Vec<Arc<dyn System>>>>,
    plugins: SharedLock<Vec<Box<dyn Plugin>>>,
    runner: Option<Box<dyn Runner>>,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let world = World::new();

        let this = Self {
            world,
            systems: SharedLock::new(FxHashMap::default()),
            plugins: SharedLock::new(Vec::new()),
            runner: None,
        };

        Ok(this)
    }

    pub fn add_plugin<T: Plugin>(&mut self, plugin: T) -> anyhow::Result<()> {
        let name = plugin.name().to_owned();
        log::debug!("Adding plugin: {:?}", &name);
        plugin.build(self)?;

        self.plugins.write().push(Box::new(plugin));

        Ok(())
    }

    pub fn set_runner<T: Runner>(&mut self, runner: T) {
        self.runner = Some(Box::new(runner));
    }

    pub fn add_resource<T: Component>(&self, resource: T) {
        self.world.insert_resource(resource)
    }

    pub fn get_resource<T: Component>(&self) -> Option<Ref<T>> {
        self.world.get_resource::<T>()
    }

    pub fn get_resource_mut<T: Component>(&self) -> Option<Mut<T>> {
        self.world.get_resource_mut::<T>()
    }

    pub fn world(&self) -> &Rc<World> {
        &self.world
    }

    pub fn root_scene(&self) -> Ref<Scene> {
        self.world.root_scene()
    }

    pub fn add_system<T: System>(&self, system: T, stage: SystemStage) -> anyhow::Result<()> {
        let system = Arc::new(system);
        self.systems.write().entry(stage).or_default().push(system);
        Ok(())
    }

    pub fn run_systems(&self, stage: SystemStage) -> anyhow::Result<()> {
        let systems = self.systems.read().get(&stage).cloned();
        if let Some(systems) = systems {
            for system in systems {
                system.run(self.world())?;
            }
        }
        Ok(())
    }

    pub fn run(mut self) -> anyhow::Result<()> {
        for plugin in self.plugins.read().iter() {
            plugin.finish(&mut self)?;
        }

        if let Some(runner) = self.runner.take() {
            runner.run(self)
        } else {
            Ok(())
        }
    }
}
