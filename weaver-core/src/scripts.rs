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
    #[allow(clippy::type_complexity)]
    pub(crate) scripts: Arc<RwLock<FxHashMap<String, (Script, Vec<(SystemStage, NodeIndex)>)>>>,
}

impl Scripts {
    pub fn new(world: Arc<RwLock<World>>) -> Self {
        let scripts = world.read().script_systems.clone();
        Self { scripts, world }
    }

    pub fn reload(&self) -> bool {
        World::reload_scripts(&self.world);
        self.world.read().system_errors().is_empty()
    }

    pub fn has_errors(&self) -> bool {
        !self.world.read().system_errors().is_empty()
    }

    pub fn script_errors(&self) -> FxHashMap<String, String> {
        self.world.read().system_errors()
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
