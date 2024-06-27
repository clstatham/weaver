use std::{
    any::TypeId,
    hash::{BuildHasherDefault, Hasher},
};

pub mod lock;

pub mod prelude {
    pub use crate::lock::*;
    pub use crate::TypeIdMap;
    pub use crate::{
        debug_once, define_atomic_id, error_once, info_once, log_once, trace_once, warn_once,
    };
    pub use anyhow::{anyhow, bail, ensure, Error, Result};
    pub use downcast_rs::{impl_downcast, Downcast, DowncastSync};
}

#[macro_export]
macro_rules! define_atomic_id {
    ($id:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $id(u64);

        impl $id {
            #[allow(clippy::new_without_default)]
            pub fn new() -> Self {
                static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                Self(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
            }
        }

        impl Into<u64> for $id {
            fn into(self) -> u64 {
                self.0
            }
        }

        impl Into<usize> for $id {
            fn into(self) -> usize {
                self.0 as usize
            }
        }
    };
}

#[macro_export]
macro_rules! log_once {
    ($log:ident; $($arg:tt)*) => {{
        use std::sync::Once;
        static LOGGED: Once = Once::new();
        LOGGED.call_once(|| {
            log::$log!($($arg)*);
        });
    }};
}

#[macro_export]
macro_rules! error_once {
    ($($arg:tt)*) => {
        $crate::log_once!(error; $($arg)*);
    };
}

#[macro_export]
macro_rules! warn_once {
    ($($arg:tt)*) => {
        $crate::log_once!(warn; $($arg)*);
    };
}

#[macro_export]
macro_rules! info_once {
    ($($arg:tt)*) => {
        $crate::log_once!(info; $($arg)*);
    };
}

#[macro_export]
macro_rules! debug_once {
    ($($arg:tt)*) => {
        $crate::log_once!(debug; $($arg)*);
    };
}

#[macro_export]
macro_rules! trace_once {
    ($($arg:tt)*) => {
        $crate::log_once!(trace; $($arg)*);
    };
}

#[derive(Default)]
pub struct TypeIdHasher {
    state: u64,
}

impl Hasher for TypeIdHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write_u128(&mut self, i: u128) {
        self.state = i as u64;
    }

    fn write_u64(&mut self, i: u64) {
        self.state = i;
    }

    fn write(&mut self, _bytes: &[u8]) {
        unimplemented!("TypeIdHasher should not be used with anything other than TypeId")
    }
}

pub type TypeIdMap<T> =
    std::collections::hash_map::HashMap<TypeId, T, BuildHasherDefault<TypeIdHasher>>;
