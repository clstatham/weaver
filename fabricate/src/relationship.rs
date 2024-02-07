use anyhow::Result;
use fabricate::storage::Data;

use crate::{
    self as fabricate,
    prelude::{Atom, Entity},
};

pub trait Relationship: Atom {
    fn into_relationship_data(self, relative: &Entity) -> Result<Data>
    where
        Self: Sized,
    {
        Ok(Data::new_relation(self, relative))
    }
}
