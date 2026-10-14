//! Log-once macros and `tracing` shorthands.

/// Logs a formatted message at the given `log` level (`error`, `warn`, ...) the first time that
/// exact message text is seen, and never again.
///
/// The set of seen messages is per call site and grows forever, so don't put unbounded data
/// (frame counters, say) in the message.
#[macro_export]
macro_rules! log_once {
    ($log:ident; $($arg:tt)*) => {{
        use std::sync::RwLock;
        use std::collections::HashSet;
        $crate::re_exports::lazy_static! {
            static ref LOGGED: RwLock<HashSet<String>> = RwLock::new(HashSet::new());
        }
        let msg = format!($($arg)*);
        if !LOGGED.read().unwrap().contains(&msg) {
            LOGGED.write().unwrap().insert(msg.clone());
            $crate::re_exports::log::$log!("{}", msg);
        }
    }};
}

/// [`log_once!`] at `error` level.
#[macro_export]
macro_rules! error_once {
    ($($arg:tt)*) => {
        $crate::log_once!(error; $($arg)*);
    };
}

/// [`log_once!`] at `warn` level.
#[macro_export]
macro_rules! warn_once {
    ($($arg:tt)*) => {
        $crate::log_once!(warn; $($arg)*);
    };
}

/// [`log_once!`] at `info` level.
#[macro_export]
macro_rules! info_once {
    ($($arg:tt)*) => {
        $crate::log_once!(info; $($arg)*);
    };
}

/// [`log_once!`] at `debug` level.
#[macro_export]
macro_rules! debug_once {
    ($($arg:tt)*) => {
        $crate::log_once!(debug; $($arg)*);
    };
}

/// [`log_once!`] at `trace` level.
#[macro_export]
macro_rules! trace_once {
    ($($arg:tt)*) => {
        $crate::log_once!(trace; $($arg)*);
    };
}

/// Shorthand for `tracing::span!` taking the level as a bare identifier, e.g. `span!(INFO, "x")`.
#[macro_export]
macro_rules! span {
    ($level:ident, $($tail:tt)*) => {{
        $crate::re_exports::tracing::span!($crate::re_exports::tracing::Level::$level, $($tail)*)
    }};
}

/// Shorthand for `tracing::event!` taking the level as a bare identifier.
#[macro_export]
macro_rules! event {
    ($level:ident, $name:expr) => {{ $crate::re_exports::tracing::event!($crate::re_exports::tracing::Level::$level, $name) }};
}
