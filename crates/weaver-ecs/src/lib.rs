#![allow(clippy::multiple_bound_locations)] // downcast-rs thing

pub mod bundle;
pub mod component;
pub mod entity;
pub mod node;
pub mod query;
pub mod relationship;
pub mod scene;
pub mod storage;
pub mod system;
pub mod world;

pub mod prelude {
    pub use crate::bundle::*;
    pub use crate::component::*;
    pub use crate::entity::*;
    pub use crate::node::*;
    pub use crate::query::*;
    pub use crate::relationship::*;
    pub use crate::scene::*;
    pub use crate::storage::*;
    pub use crate::system::*;
    pub use crate::world::*;
    pub use weaver_ecs_macros::*;
}
