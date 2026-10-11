//! Plugin lifecycle.
//!
//! Tests marked `#[ignore = "known issue: ..."]` assert the intended behavior and currently fail.
//! Run them with `cargo test -p weaver-app -- --ignored`.

use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

use weaver_app::prelude::*;
use weaver_ecs::prelude::Commands;
use weaver_util::prelude::Result;

static COUNTED_RUNS: AtomicUsize = AtomicUsize::new(0);

fn counted(_commands: Commands) {
    COUNTED_RUNS.fetch_add(1, Ordering::SeqCst);
}

struct CountingPlugin;

impl Plugin for CountingPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_system(counted, Update);
        Ok(())
    }
}

/// `configure_plugin` runs `cleanup` and then `build` again. Systems added by the first `build`
/// must not stay registered alongside the second set.
#[test]
fn configure_plugin_does_not_duplicate_systems() {
    let mut app = App::new();
    app.add_plugin(CountingPlugin).unwrap();
    app.configure_plugin::<CountingPlugin>(|_| {});
    app.init();
    app.update();

    assert_eq!(COUNTED_RUNS.load(Ordering::SeqCst), 1);
}

static INNER_FINISHED: AtomicBool = AtomicBool::new(false);

struct Inner;

impl Plugin for Inner {
    fn build(&self, _app: &mut App) -> Result<()> {
        Ok(())
    }

    fn finish(&self, _app: &mut App) -> Result<()> {
        INNER_FINISHED.store(true, Ordering::SeqCst);
        Ok(())
    }
}

struct Outer;

impl Plugin for Outer {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(Inner)?;
        Ok(())
    }
}

/// A plugin added from inside another plugin's `build` on a `SubApp` must be finished like any other.
#[test]
#[ignore = "known issue: SubApp builds plugins against a temporary App whose plugin list is discarded"]
fn nested_plugins_in_sub_app_are_finished() {
    let mut sub_app = SubApp::new();
    sub_app.add_plugin(Outer).unwrap();
    sub_app.finish_plugins();

    assert!(INNER_FINISHED.load(Ordering::SeqCst));
}

struct NeverReady;

impl Plugin for NeverReady {
    fn build(&self, _app: &mut App) -> Result<()> {
        Ok(())
    }

    fn ready(&self, _app: &App) -> bool {
        false
    }
}

/// Nothing can change between iterations of `App::update`'s wait loop, so a plugin that isn't
/// ready must not make `update` spin forever.
#[test]
#[ignore = "known issue: App::update busy-loops until every plugin is ready"]
fn update_returns_while_a_plugin_is_not_ready() {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut app = App::new();
        app.add_plugin(NeverReady).unwrap();
        app.init();
        app.update();
        let _ = tx.send(());
    });

    rx.recv_timeout(Duration::from_secs(3))
        .expect("App::update never returned");
}
