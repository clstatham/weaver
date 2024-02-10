use anyhow::Result;
use fabricate::storage::Data;

use crate::{
    self as fabricate,
    prelude::{Component, Entity},
};

pub trait Relationship: Component {
    fn into_relationship_data(self, relative: Entity) -> Result<Data>
    where
        Self: Sized,
    {
        Ok(Data::new_relationship(self, relative))
    }
}
