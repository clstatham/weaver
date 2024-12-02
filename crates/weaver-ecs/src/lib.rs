#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(non_snake_case)]

pub mod bundle;
pub mod commands;
pub mod component;
pub mod entity;
pub mod loan;
pub mod query;
pub mod storage;
pub mod system;
pub mod system_schedule;
pub mod world;

pub use tokio::main;
pub use weaver_ecs_macros::*;

pub mod prelude {
    pub use crate::bundle::*;
    pub use crate::commands::*;
    pub use crate::component::*;
    pub use crate::entity::*;
    pub use crate::loan::*;
    pub use crate::query::*;
    pub use crate::storage::*;
    pub use crate::system::*;
    pub use crate::system_schedule::*;
    pub use crate::world::*;
    pub use crate::SystemStage;
    pub use tokio;
}
