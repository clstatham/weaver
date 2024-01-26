use std::fmt::Debug;
use std::sync::Arc;

use crate as weaver_ecs;
use crate::prelude::EntityGraph;
use crate::storage::TemporaryComponents;
use crate::{bundle::Bundle, entity::Entity, world::World};
use parking_lot::RwLock;
use weaver_proc_macro::Component;

#[derive(Component)]
pub struct Commands {
    created_components: TemporaryComponents,
    despawned_entities: Vec<Entity>,
    recursive_despawned_entities: Vec<Entity>,
}

impl Commands {
    pub fn new(world: Arc<RwLock<World>>) -> Self {
        Self {
            created_components: world.read().components.split(),
            despawned_entities: Vec::new(),
            recursive_despawned_entities: Vec::new(),
        }
    }

    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> Entity {
        bundle.build(&mut self.created_components.components)
    }

    pub fn despawn(&mut self, entity: Entity) {
        self.despawned_entities.push(entity);
    }

    pub fn despawn_recursive(&mut self, entity: Entity) {
        self.recursive_despawned_entities.push(entity);
    }

    pub fn finalize(self, world: &mut World) {
        for entity in self.created_components.components.living_entities() {
            world
                .write_resource::<EntityGraph>()
                .unwrap()
                .add_entity(*entity);
        }
        world.components.merge(self.created_components);

        for entity in self.despawned_entities {
            world.despawn(entity);
        }

        for entity in self.recursive_despawned_entities {
            world.despawn_recursive(entity);
        }
    }
}

impl Debug for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Commands").finish()
    }
}
