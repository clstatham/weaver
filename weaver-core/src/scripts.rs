use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};
use petgraph::stable_graph::NodeIndex;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use weaver_ecs::{prelude::*, script::Script};

#[derive(Component)]
pub struct Scripts {
    world: Arc<RwLock<World>>,
    pub(crate) scripts: Arc<RwLock<FxHashMap<String, (Script, Vec<(SystemStage, NodeIndex)>)>>>,
    script_errors: Arc<RwLock<FxHashMap<String, String>>>,
}

impl Scripts {
    pub fn new(world: Arc<RwLock<World>>) -> Self {
        let scripts = world.read().script_systems.clone();
        Self {
            scripts,
            world,
            script_errors: Arc::new(RwLock::new(FxHashMap::default())),
        }
    }

    pub fn reload(&self) -> bool {
        self.script_errors.write().clear();
        World::reload_scripts(&self.world).unwrap_or_else(|e| {
            log::error!("Failed to reload scripts: {:?}", &e);
            self.script_errors.write().extend(e);
        });
        self.script_errors.read().is_empty()
    }

    pub fn script_errors(&self, name: &str) -> Option<String> {
        self.script_errors.read().get(name).cloned()
    }

    pub fn script(&self, name: &str) -> MappedRwLockReadGuard<'_, Script> {
        let scripts = self.scripts.read();
        RwLockReadGuard::map(scripts, |scripts| scripts.get(name).map(|s| &s.0).unwrap())
    }

    pub fn script_mut(&self, name: &str) -> MappedRwLockWriteGuard<'_, Script> {
        let scripts = self.scripts.write();
        RwLockWriteGuard::map(scripts, |scripts| {
            scripts.get_mut(name).map(|s| &mut s.0).unwrap()
        })
    }

    pub fn script_iter(&self) -> impl Iterator<Item = MappedRwLockReadGuard<'_, Script>> {
        let script_names = self.scripts.read().keys().cloned().collect::<Vec<_>>();
        script_names.into_iter().map(move |name| self.script(&name))
    }

    pub fn script_iter_mut(&self) -> impl Iterator<Item = MappedRwLockWriteGuard<'_, Script>> {
        let script_names = self.scripts.read().keys().cloned().collect::<Vec<_>>();
        script_names
            .into_iter()
            .map(move |name| self.script_mut(&name))
    }
}
