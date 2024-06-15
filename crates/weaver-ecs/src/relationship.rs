use std::sync::Arc;

use weaver_util::prelude::{impl_downcast, DowncastSync};

use super::node::Node;

pub trait Relationship: DowncastSync {}
impl_downcast!(Relationship);

pub struct RelationshipConnection {
    pub from: Node,
    pub to: Node,
    pub weight: Arc<dyn Relationship>,
}

impl RelationshipConnection {
    pub fn new<T: Relationship>(from: Node, to: Node, weight: T) -> Self {
        Self {
            from,
            to,
            weight: Arc::new(weight),
        }
    }
}
