//! Small shared utilities that every other weaver crate builds on.
//!
//! This crate depends on no other workspace crate. It holds the lock wrappers the ECS uses for
//! borrow tracking ([`lock::Lock`], [`lock::SharedLock`] and their guards), [`lock::SyncCell`],
//! string and label interning ([`intern`], [`label`]), the hash map aliases in [`maps`], a sorted
//! vector, the [`define_atomic_id!`] macro, log-once macros, and re-exports of the third-party
//! crates the engine uses everywhere (`anyhow`, `hashbrown`, `log`, `tracing` and friends).
//!
//! Most crates just do `use weaver_util::prelude::*;`, which brings in the lot.

pub mod atomic_id;
pub mod intern;
#[macro_use]
pub mod label;
pub mod lock;
#[macro_use]
pub mod logging;
pub mod maps;
pub mod sorted_vec;

/// Third-party items the rest of the engine imports through this crate, so versions stay in one
/// place.
pub mod re_exports {
    pub use anyhow::{Error, Result, anyhow, bail, ensure};
    pub use downcast_rs::{Downcast, DowncastSync, impl_downcast};
    pub use hashbrown::{HashMap, HashSet};
    pub use indextree;
    pub use lazy_static::lazy_static;
    pub use log;
    pub use rustc_hash::FxHasher;
    pub use scopeguard::{ScopeGuard, defer, guard};
    pub use thiserror::Error;
    pub use tracing;
}

/// Glob-import this to get the error types, locks, maps, interning and macros in scope.
///
/// Note that [`Result`](re_exports::Result) here is `anyhow::Result`.
pub mod prelude {
    pub use crate::{
        debug_once, define_atomic_id, define_label, error_once, info_once,
        intern::*,
        lock::*,
        log_once,
        maps::{FxHashMap, FxHashSet, TypeIdMap, TypeIdSet},
        re_exports::*,
        sorted_vec::SortedVec,
        trace_once, warn_once,
    };
}
