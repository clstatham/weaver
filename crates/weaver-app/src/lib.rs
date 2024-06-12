use std::sync::Arc;

use plugin::Plugin;
use rustc_hash::FxHashMap;
use weaver_ecs::{
    bundle::Bundle,
    component::Resource,
    entity::Entity,
    scene::Scene,
    storage::Ref,
    system::{FunctionSystem, Res, ResMut, System, SystemStage},
    world::World,
};
use weaver_reflect::registry::{TypeRegistry, Typed};
use weaver_util::lock::SharedLock;

pub mod plugin;

pub mod prelude {
    pub use crate::plugin::Plugin;
    pub use crate::App;
}

pub trait Runner: 'static {
    fn run(&self, app: &mut App) -> anyhow::Result<()>;
}

impl<T> Runner for T
where
    T: Fn(&mut App) -> anyhow::Result<()> + Send + Sync + 'static,
{
    fn run(&self, app: &mut App) -> anyhow::Result<()> {
        self(app)
    }
}

pub struct App {
    world: Arc<World>,
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

        this.add_resource(TypeRegistry::new());

        Ok(this)
    }

    pub fn add_plugin<T: Plugin>(&mut self, plugin: T) -> anyhow::Result<&mut Self> {
        let name = plugin.name().to_owned();
        log::debug!("Adding plugin: {:?}", &name);
        plugin.build(self)?;

        self.plugins.write().push(Box::new(plugin));

        Ok(self)
    }

    pub fn set_runner<T: Runner>(&mut self, runner: T) {
        self.runner = Some(Box::new(runner));
    }

    pub fn register_type<T: Typed>(&self) {
        self.get_resource_mut::<TypeRegistry>()
            .unwrap()
            .register::<T>();
    }

    pub fn add_resource<T: Resource>(&self, resource: T) -> &Self {
        self.world.insert_resource(resource);
        self
    }

    pub fn get_resource<T: Resource>(&self) -> Option<Res<T>> {
        self.world.get_resource::<T>()
    }

    pub fn get_resource_mut<T: Resource>(&self) -> Option<ResMut<T>> {
        self.world.get_resource_mut::<T>()
    }

    pub fn spawn<T: Bundle>(&self, bundle: T) -> Entity {
        self.world().spawn(bundle)
    }

    pub fn world(&self) -> &Arc<World> {
        &self.world
    }

    pub fn root_scene(&self) -> Ref<Scene> {
        self.world.root_scene()
    }

    pub fn add_system<M>(
        &mut self,
        system: impl FunctionSystem<M> + 'static,
        stage: SystemStage,
    ) -> anyhow::Result<&mut Self> {
        self.systems
            .write()
            .entry(stage)
            .or_default()
            .push(system.into_system());
        Ok(self)
    }

    pub fn run_systems(&self, stage: SystemStage) -> anyhow::Result<()> {
        let systems = self.systems.read().get(&stage).cloned();
        if let Some(systems) = systems {
            for system in systems {
                system.run(self.world().clone())?;
            }
        }
        Ok(())
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        for plugin in self.plugins.read().iter() {
            plugin.finish(self)?;
        }

        if let Some(runner) = self.runner.take() {
            runner.run(self)
        } else {
            Ok(())
        }
    }
}
