use crate as weaver_ecs;
use crate::storage::TemporaryComponents;
use crate::{bundle::Bundle, entity::Entity, world::World};
use weaver_proc_macro::Resource;

#[derive(Resource)]
pub struct Commands {
    created_components: TemporaryComponents,
    despawned_entities: Vec<Entity>,
}

impl Commands {
    pub fn new(world: &World) -> Self {
        Self {
            created_components: world.components.split(),
            despawned_entities: Vec::new(),
        }
    }

    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> Entity {
        bundle.build(&mut self.created_components.components)
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
