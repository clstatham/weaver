#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(non_snake_case)]

pub mod bundle;
pub mod change_detection;
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
    pub use futures::{self, StreamExt};
    pub use tokio;
}

pub fn spin_on<F, R>(f: F) -> R
where
    F: std::future::Future<Output = R>,
{
    use futures::future::FutureExt;
    tokio::pin!(f);
    let mut cx = futures::task::Context::from_waker(futures::task::noop_waker_ref());
    loop {
        if let futures::task::Poll::Ready(result) = f.poll_unpin(&mut cx) {
            return result;
        }

        std::thread::yield_now();
    }
}
