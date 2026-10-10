//! Asset loading through the `AssetLoad` stage.
//!
//! Tests marked `#[ignore = "known issue: ..."]` assert the intended behavior and currently fail.
//! Run them with `cargo test -p weaver-asset -- --ignored`.

use std::{sync::mpsc, time::Duration};

use weaver_app::SubApp;
use weaver_asset::{AssetApp, prelude::*};
use weaver_task::{task_pool::TaskPool, usages::GlobalTaskPool};

struct Blob;
impl Asset for Blob {}

const COUNT: usize = 1100;

/// Every queued asset should load in one update, however many there are. `load_all_assets`
/// sends one `AssetLoaded` event per asset; these used to go into a bounded channel that nothing
/// cleared, so the 1025th send never completed and the update hung.
#[test]
fn loading_more_than_1024_assets_completes() {
    GlobalTaskPool::get_or_init(TaskPool::new);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut app = SubApp::new();
        app.add_asset::<Blob>();
        let handles: Vec<Handle<Blob>> = {
            let queue = app
                .world()
                .get_resource::<AssetLoadQueue<DirectLoader<Blob>, Blob>>()
                .unwrap();
            (0..COUNT).map(|_| queue.enqueue(Blob)).collect()
        };
        app.world_mut().initialize_systems();
        app.world_mut().update().unwrap();

        let mut assets = app.world().get_resource_mut::<Assets<Blob>>().unwrap();
        let loaded = handles
            .into_iter()
            .filter(|handle| assets.get(*handle).is_some())
            .count();
        let _ = tx.send(loaded);
    });

    let loaded = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the AssetLoad stage hung");
    assert_eq!(loaded, COUNT);
}
