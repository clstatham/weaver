#![allow(clippy::multiple_bound_locations)] // downcast-rs thing

pub mod bundle;
pub mod change;
pub mod component;
pub mod entity;
pub mod node;
pub mod query;
pub mod reflect;
pub mod relationship;
pub mod scene;
pub mod storage;
pub mod world;

pub mod prelude {
    pub use crate::bundle::*;
    pub use crate::change::*;
    pub use crate::component::*;
    pub use crate::entity::*;
    pub use crate::node::*;
    pub use crate::query::*;
    pub use crate::relationship::*;
    pub use crate::scene::*;
    pub use crate::storage::*;
    pub use crate::world::*;
    pub use weaver_ecs_macros::*;
    pub use weaver_reflect_macros::*;
}
