use fabricate::storage::Data;

use crate::{
    self as fabricate,
    prelude::{Component, Entity},
    world::LockedWorldHandle,
};

pub trait Relationship: Component {
    fn into_relationship_data(self, world: &LockedWorldHandle, relative: Entity) -> Data
    where
        Self: Sized,
    {
        Data::new_relationship(world, self, relative)
    }
}
