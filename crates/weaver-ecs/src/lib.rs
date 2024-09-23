#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(non_snake_case)]

pub mod bundle;
pub mod change;
pub mod commands;
pub mod component;
pub mod entity;
pub mod query;
pub mod reflect;
pub mod storage;
pub mod system;
pub mod system_schedule;
pub mod world;
pub mod world_view;

pub mod prelude {
    pub use crate::bundle::*;
    pub use crate::change::*;
    pub use crate::commands::*;
    pub use crate::component::*;
    pub use crate::entity::*;
    pub use crate::query::*;
    pub use crate::reflect::{registry::*, *};
    pub use crate::storage::*;
    pub use crate::system::*;
    pub use crate::system_schedule::*;
    pub use crate::world::*;
    pub use crate::world_view::*;
    pub use weaver_ecs_macros::*;
    pub use weaver_reflect_macros::*;
}
