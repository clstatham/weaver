//! `AssetEvent<T>`: added / modified / removed notifications.

use weaver_app::SubApp;
use weaver_asset::{AssetApp, AssetEvent, AssetEventKind, AssetId, prelude::*};
use weaver_ecs::{change_detection::TickRange, system::SystemParam};
use weaver_event::{EventRx, EventRxState};
use weaver_task::{task_pool::TaskPool, usages::GlobalTaskPool};

struct Blob(u32);
impl Asset for Blob {}

fn read(app: &SubApp, state: &EventRxState<AssetEvent<Blob>>) -> Vec<(AssetId, AssetEventKind)> {
    let world = app.world();
    let ticks: TickRange = world.world_ticks();
    <EventRx<AssetEvent<Blob>> as SystemParam>::fetch(world, state, ticks)
        .map(|event| (event.id, event.kind))
        .collect()
}

#[test]
fn asset_changes_are_reported_once_each() {
    GlobalTaskPool::get_or_init(TaskPool::new);

    let mut app = SubApp::new();
    app.add_asset::<Blob>();
    app.world_mut().initialize_systems();
    let state = EventRxState::default();

    let (a, b) = {
        let mut assets = app.world().get_resource_mut::<Assets<Blob>>().unwrap();
        (assets.insert(Blob(1)), assets.insert(Blob(2)))
    };
    app.world_mut().update().unwrap();
    assert_eq!(
        read(&app, &state),
        vec![
            (a.id(), AssetEventKind::Added),
            (b.id(), AssetEventKind::Added)
        ]
    );

    // reading doesn't count as a modification
    {
        let assets = app.world().get_resource::<Assets<Blob>>().unwrap();
        assert_eq!(assets.get(a).unwrap().0, 1);
    }
    app.world_mut().update().unwrap();
    assert_eq!(read(&app, &state), vec![]);

    {
        let mut assets = app.world().get_resource_mut::<Assets<Blob>>().unwrap();
        assets.get_mut(a).unwrap().0 = 10;
        assets.remove(b);
    }
    app.world_mut().update().unwrap();
    assert_eq!(
        read(&app, &state),
        vec![
            (a.id(), AssetEventKind::Modified),
            (b.id(), AssetEventKind::Removed)
        ]
    );
}
