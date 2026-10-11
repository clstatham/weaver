//! Run conditions (`run_if`).

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};

use common::init_task_pool;
use weaver_ecs::{change_detection::TickRange, prelude::*};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
struct Stage1;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
struct Stage2;

struct Gate;
struct Counter(u32);

static GATED_RUNS: AtomicUsize = AtomicUsize::new(0);

fn gated(_commands: Commands) {
    GATED_RUNS.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn resource_exists_gates_a_system() {
    init_task_pool();
    let mut world = World::new();
    world.push_update_stage(Stage1);
    world.add_system(gated.run_if(resource_exists::<Gate>()), Stage1);
    world.initialize_systems();

    world.update().unwrap();
    assert_eq!(GATED_RUNS.load(Ordering::SeqCst), 0);

    world.insert_resource(Gate);
    world.update().unwrap();
    world.update().unwrap();
    assert_eq!(GATED_RUNS.load(Ordering::SeqCst), 2);

    world.remove_resource::<Gate>();
    world.update().unwrap();
    assert_eq!(GATED_RUNS.load(Ordering::SeqCst), 2);
}

static CHANGED_RUNS: AtomicUsize = AtomicUsize::new(0);

fn bump_every_other_frame(mut counter: ResMut<Counter>, mut frame: Local<u32>) {
    let mut frame = frame.get_mut();
    if frame.is_multiple_of(2) {
        counter.0 += 1;
    }
    *frame += 1;
}

fn on_counter_changed(_counter: Res<Counter>) {
    CHANGED_RUNS.fetch_add(1, Ordering::SeqCst);
}

/// `resource_changed` compares against the gated system's own last run, so a change made by an
/// earlier stage is seen exactly once.
#[test]
fn resource_changed_runs_once_per_change() {
    init_task_pool();
    let mut world = World::new();
    world.insert_resource(Counter(0));
    world.push_update_stage(Stage1);
    world.push_update_stage(Stage2);
    world.add_system(bump_every_other_frame, Stage1);
    world.add_system(
        on_counter_changed.run_if(resource_changed::<Counter>()),
        Stage2,
    );
    world.initialize_systems();

    for _ in 0..6 {
        world.update().unwrap();
    }

    // changed on frames 0, 2 and 4
    assert_eq!(CHANGED_RUNS.load(Ordering::SeqCst), 3);
}

static CUSTOM_RUNS: AtomicUsize = AtomicUsize::new(0);

fn custom(_commands: Commands) {
    CUSTOM_RUNS.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn closures_and_not_work_as_conditions() {
    init_task_pool();
    let mut world = World::new();
    world.push_update_stage(Stage1);
    world.add_system(
        custom
            .run_if(|world: &World, _: TickRange| world.has_resource::<Counter>())
            .run_if(not(resource_exists::<Gate>())),
        Stage1,
    );
    world.initialize_systems();

    world.update().unwrap();
    world.insert_resource(Counter(0));
    world.update().unwrap();
    world.insert_resource(Gate);
    world.update().unwrap();

    assert_eq!(CUSTOM_RUNS.load(Ordering::SeqCst), 1);
}
