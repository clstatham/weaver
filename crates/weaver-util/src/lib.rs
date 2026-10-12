pub mod atomic_id;
pub mod intern;
#[macro_use]
pub mod label;
pub mod lock;
#[macro_use]
pub mod logging;
pub mod maps;
pub mod sorted_vec;

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
