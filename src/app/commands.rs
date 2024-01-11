use crate::ecs::{
    entity::Entity,
    resource::{Res, Resource},
    Bundle, World,
};

pub struct Commands<'a> {
    world: &'a World,
}

impl<'a> Commands<'a> {
    pub fn new(world: &'a World) -> Self {
        Self { world }
    }

    pub fn spawn<T: Bundle>(&self, bundle: T) -> anyhow::Result<Entity> {
        bundle.build(self.world)
    }

    pub fn despawn(&self, entity: Entity) {
        self.world.remove_entity(entity);
    }

    pub fn read_resource<T: Resource>(&self) -> anyhow::Result<Res<T>> {
        self.world.read_resource()
    }
}
