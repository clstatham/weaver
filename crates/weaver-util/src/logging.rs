#[macro_export]
macro_rules! log_once {
    ($log:ident; $($arg:tt)*) => {{
        use std::sync::RwLock;
        use std::collections::HashSet;
        weaver_util::re_exports::lazy_static! {
            static ref LOGGED: RwLock<HashSet<String>> = RwLock::new(HashSet::new());
        }
        let msg = format!($($arg)*);
        if !LOGGED.read().unwrap().contains(&msg) {
            LOGGED.write().unwrap().insert(msg.clone());
            log::$log!("{}", msg);
        }
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
