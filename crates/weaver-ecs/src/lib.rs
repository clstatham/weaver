#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::unused_async)]

pub mod bundle;
pub mod change_detection;
pub mod commands;
pub mod component;
mod component_vec;
pub mod condition;
pub mod entity;
pub mod loan;
pub mod query;
pub mod removal;
pub mod storage;
pub mod system;
pub mod system_schedule;
pub mod world;

pub use weaver_ecs_macros::*;

pub mod prelude {
    pub use crate::SystemStage;
    pub use crate::bundle::*;
    pub use crate::commands::*;
    pub use crate::component::*;
    pub use crate::condition::{
        any_with_component, not, resource_added, resource_changed, resource_exists,
    };
    pub use crate::entity::*;
    pub use crate::loan::*;
    pub use crate::query::*;
    pub use crate::removal::RemovedComponents;
    pub use crate::storage::*;
    pub use crate::system::*;
    pub use crate::system_schedule::*;
    pub use crate::world::*;
}
