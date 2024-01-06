use weaver_proc_macro::impl_bundle_for_tuple;

use super::{Component, Entity, World};

/// A collection of components that can be built and added to an entity.
pub trait Bundle {
    fn build(self, world: &World) -> anyhow::Result<Entity>;
}

impl Bundle for () {
    fn build(self, world: &World) -> anyhow::Result<Entity> {
        Ok(world.create_entity())
    }
}

impl<T: Component> Bundle for T {
    fn build(self, world: &World) -> anyhow::Result<Entity> {
        let entity = world.create_entity();
        world.add_component(entity, self)?;
        Ok(entity)
    }
}

impl<A: Component> Bundle for (A,) {
    fn build(self, world: &World) -> anyhow::Result<Entity> {
        let entity = world.create_entity();
        world.add_component(entity, self.0)?;
        Ok(entity)
    }
}

impl_bundle_for_tuple!((A, B));
impl_bundle_for_tuple!((A, B, C));
impl_bundle_for_tuple!((A, B, C, D));
impl_bundle_for_tuple!((A, B, C, D, E));
impl_bundle_for_tuple!((A, B, C, D, E, F));
impl_bundle_for_tuple!((A, B, C, D, E, F, G));
impl_bundle_for_tuple!((A, B, C, D, E, F, G, H));
