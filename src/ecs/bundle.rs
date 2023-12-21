use crate::ecs::{entity::Entity, world::World};

use super::component::Component;

pub trait Bundle
where
    Self: Send + Sync + Sized + 'static,
{
    fn build(self, world: &mut World) -> Entity;
}

macro_rules! impl_bundle_for_tuple {
    ($head:ident, $($tail:ident,)*) => {
        #[allow(unused_variables, non_snake_case)]
        impl<$head: Component, $($tail: Component,)*> Bundle for ($head, $($tail),*)
        {
            fn build(self, world: &mut World) -> Entity {
                let (head, $($tail),*) = self;
                let entity = world.spawn(head);
                $(world.add_component(entity, $tail);)*
                entity
            }
        }
    };
}

impl_bundle_for_tuple!(A,);
impl_bundle_for_tuple!(A, B,);
impl_bundle_for_tuple!(A, B, C,);
impl_bundle_for_tuple!(A, B, C, D,);
