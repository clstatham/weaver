use weaver_util::prelude::{impl_downcast, Downcast};

use super::node::Node;

pub trait Relationship: Downcast {}
impl_downcast!(Relationship);

pub struct RelationshipConnection {
    pub from: Node,
    pub to: Node,
    pub weight: Box<dyn Relationship>,
}

impl RelationshipConnection {
    pub fn new<T: Relationship>(from: Node, to: Node, weight: T) -> Self {
        Self {
            from,
            to,
            weight: Box::new(weight),
        }
    }
}
