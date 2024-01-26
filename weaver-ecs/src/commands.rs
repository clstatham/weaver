use std::fmt::Debug;
use std::sync::Arc;

use crate as weaver_ecs;
use crate::storage::TemporaryComponents;
use crate::{bundle::Bundle, entity::Entity, world::World};
use parking_lot::RwLock;
use weaver_proc_macro::Component;

#[derive(Component)]
pub struct Commands {
    world: Arc<RwLock<World>>,
    created_components: TemporaryComponents,
    despawned_entities: Vec<Entity>,
}

impl Commands {
    pub fn new(world: Arc<RwLock<World>>) -> Self {
        Self {
            world: world.clone(),
            created_components: world.read().components.split(),
            despawned_entities: Vec::new(),
        }
    }

    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> Entity {
        bundle.build(&mut self.created_components.components)
    }

    pub fn reload_scripts(&self) {
        World::reload_scripts(&self.world);
    }

    pub fn despawn(&mut self, entity: Entity) {
        self.despawned_entities.push(entity);
    }

    pub fn finalize(self, world: &mut World) {
        world.components.merge(self.created_components);

        for entity in self.despawned_entities {
            world.despawn(entity);
        }
    }
}

impl Debug for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Commands").finish()
    }
}
