pub mod atomic_id;
pub mod lock;
pub mod logging;
pub mod maps;

pub use crate::lock::*;
pub use crate::maps::{FxHashMap, FxHashSet, TypeIdMap};
pub use anyhow::{anyhow, bail, ensure, Error, Result};
pub use downcast_rs::{impl_downcast, Downcast, DowncastSync};
pub use hashbrown::{HashMap, HashSet};
pub use indextree;
pub use lazy_static::lazy_static;
pub use rustc_hash::FxHasher;
pub use scopeguard::{defer, guard, ScopeGuard};
pub use thiserror::Error;

pub mod prelude {
    pub use crate::{
        anyhow, bail, debug_once, define_atomic_id, ensure, error_once, indextree, info_once,
        lock::*,
        log_once,
        maps::{FxHashMap, FxHashSet, TypeIdMap},
        trace_once, warn_once, Downcast, DowncastSync, Error, Result, SyncCell,
    };
}
