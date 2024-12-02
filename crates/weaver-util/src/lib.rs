pub mod atomic_id;
pub mod intern;
#[macro_use]
pub mod label;
pub mod lock;
pub mod logging;
pub mod maps;
pub mod sorted_vec;

pub mod re_exports {
    pub use anyhow::{anyhow, bail, ensure, Error, Result};
    pub use downcast_rs::{impl_downcast, Downcast, DowncastSync};
    pub use hashbrown::{HashMap, HashSet};
    pub use indextree;
    pub use lazy_static::lazy_static;
    pub use rustc_hash::FxHasher;
    pub use scopeguard::{defer, guard, ScopeGuard};
    pub use thiserror::Error;
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
