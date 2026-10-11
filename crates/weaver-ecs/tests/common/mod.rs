#![allow(dead_code)]

use std::{sync::mpsc, time::Duration};

/// Runs `f` on its own thread and returns `None` if it hasn't finished after `secs` seconds,
/// which in these tests means it deadlocked. The stuck thread is leaked.
pub fn with_timeout<T: Send + 'static>(
    secs: u64,
    f: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs)).ok()
}
