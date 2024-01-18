use std::{
    any::{type_name, TypeId},
    sync::Arc,
};

use parking_lot::RwLock;

use crate::{storage::Components, world::ComponentPtr};

use super::{Component, Entity};

/// A collection of components that can be built and added to an entity.
pub trait Bundle: Sized {
    fn build(self, world: &mut Components) -> Entity {
        self.build_on(world.create_entity(), world)
    }
    fn build_on(self, entity: Entity, world: &mut Components) -> Entity;
}

impl Bundle for () {
    fn build(self, world: &mut Components) -> Entity {
        world.create_entity()
    }
    fn build_on(self, entity: Entity, _world: &mut Components) -> Entity {
        entity
    }
}

impl<T: Component> Bundle for T {
    fn build_on(self, entity: Entity, world: &mut Components) -> Entity {
        world.add_component(
            entity.id(),
            ComponentPtr {
                component_id: TypeId::of::<T>(),
                component_name: type_name::<T>().into(),
                component: Arc::new(RwLock::new(self)),
            },
        );
        entity
    }
}

impl<A: Bundle> Bundle for (A,) {
    fn build_on(self, entity: Entity, world: &mut Components) -> Entity {
        let (a,) = self;
        a.build_on(entity, world);
        entity
    }
}

macro_rules! impl_bundle_for_tuple {
    (($($name:ident),*)) => {
        impl<$($name: Bundle),*> Bundle for ($($name,)*) {
            #[allow(non_snake_case)]
            fn build_on(self, entity: Entity, world: &mut Components) -> Entity {
                let ($($name,)*) = self;
                $(
                    $name.build_on(entity, world);
                )*
                entity
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
