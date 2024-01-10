use super::{Component, Entity, World};

/// A collection of components that can be built and added to an entity.
pub trait Bundle: Sized {
    fn build(self, world: &World) -> anyhow::Result<Entity> {
        self.build_on(world.create_entity(), world)
    }
    fn build_on(self, entity: Entity, world: &World) -> anyhow::Result<Entity>;
}

impl Bundle for () {
    fn build(self, world: &World) -> anyhow::Result<Entity> {
        Ok(world.create_entity())
    }
    fn build_on(self, entity: Entity, _world: &World) -> anyhow::Result<Entity> {
        Ok(entity)
    }
}

impl<T: Component> Bundle for T {
    fn build_on(self, entity: Entity, world: &World) -> anyhow::Result<Entity> {
        world.add_component(entity, self)?;
        Ok(entity)
    }
}

impl<A: Bundle> Bundle for (A,) {
    fn build_on(self, entity: Entity, world: &World) -> anyhow::Result<Entity> {
        let (a,) = self;
        a.build_on(entity, world)?;
        Ok(entity)
    }
}

macro_rules! impl_bundle_for_tuple {
    (($($name:ident),*)) => {
        impl<$($name: Bundle),*> Bundle for ($($name,)*) {
            #[allow(non_snake_case)]
            fn build_on(self, entity: Entity, world: &World) -> anyhow::Result<Entity> {
                let ($($name,)*) = self;
                $(
                    $name.build_on(entity, world)?;
                )*
                Ok(entity)
            }
        }
    };
}

impl_bundle_for_tuple!((A, B));
impl_bundle_for_tuple!((A, B, C));
impl_bundle_for_tuple!((A, B, C, D));
impl_bundle_for_tuple!((A, B, C, D, E));
impl_bundle_for_tuple!((A, B, C, D, E, F));
impl_bundle_for_tuple!((A, B, C, D, E, F, G));
impl_bundle_for_tuple!((A, B, C, D, E, F, G, H));
