//! Change detection (`is_changed` / `is_added` / `Changed<>`).
//!
//! Tests marked `#[ignore = "known issue: ..."]` assert the intended behavior and currently fail.
//! Run them with `cargo test -p weaver-ecs -- --ignored`.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};

use common::init_task_pool;
use weaver_ecs::{change_detection::ChangeDetection, prelude::*};

struct A(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
struct Stage1;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
struct Stage2;

const ENTITIES: usize = 10;

static LATER_SAW_CHANGED: AtomicUsize = AtomicUsize::new(usize::MAX);

async fn mutate_all(mut query: Query<&mut A>) {
    for mut a in query.iter() {
        a.0 += 1;
    }
}

async fn count_changed_later(mut query: Query<&A>) {
    let changed = query.iter().filter(|a| a.is_changed()).count();
    LATER_SAW_CHANGED.store(changed, Ordering::SeqCst);
}

/// A system must see changes made by a system that ran before it.
#[test]
#[ignore = "known issue: ticks are world-global, so only changes stamped with the reader's own tick count"]
fn later_system_sees_changes_from_earlier_stage() {
    init_task_pool();
    let mut world = World::new();
    for i in 0..ENTITIES as u32 {
        world.spawn((A(i),));
    }
    world.push_update_stage(Stage1);
    world.push_update_stage(Stage2);
    world.add_system(mutate_all, Stage1);
    world.add_system(count_changed_later, Stage2);
    world.initialize_systems();

    for frame in 0..3 {
        world.update().unwrap();
        assert_eq!(
            LATER_SAW_CHANGED.load(Ordering::SeqCst),
            ENTITIES,
            "frame {frame}"
        );
    }
}

static SELF_SAW_CHANGED: AtomicUsize = AtomicUsize::new(usize::MAX);

async fn mutate_and_count(mut query: Query<&mut A>) {
    for mut a in query.iter() {
        a.0 += 1;
    }
    let changed = query.iter().filter(|a| a.is_changed()).count();
    SELF_SAW_CHANGED.store(changed, Ordering::SeqCst);
}

/// A system that has just mutated a component must see it as changed, on the first frame too.
#[test]
#[ignore = "known issue: on the first update both ticks are 0, so nothing counts as changed"]
fn system_sees_its_own_changes_on_first_update() {
    init_task_pool();
    let mut world = World::new();
    for i in 0..ENTITIES as u32 {
        world.spawn((A(i),));
    }
    world.push_update_stage(Stage1);
    world.add_system(mutate_and_count, Stage1);
    world.initialize_systems();

    world.update().unwrap();

    assert_eq!(SELF_SAW_CHANGED.load(Ordering::SeqCst), ENTITIES);
}

static SAW_ADDED: AtomicUsize = AtomicUsize::new(usize::MAX);

async fn count_added(mut query: Query<&A>) {
    let added = query.iter().filter(|a| a.is_added()).count();
    SAW_ADDED.store(added, Ordering::SeqCst);
}

/// Components that existed before a system's first run must be reported as added on that run.
#[test]
#[ignore = "known issue: on the first update both ticks are 0, so nothing counts as added"]
fn components_spawned_before_first_run_are_added() {
    init_task_pool();
    let mut world = World::new();
    for i in 0..ENTITIES as u32 {
        world.spawn((A(i),));
    }
    world.push_update_stage(Stage1);
    world.add_system(count_added, Stage1);
    world.initialize_systems();

    world.update().unwrap();

    assert_eq!(SAW_ADDED.load(Ordering::SeqCst), ENTITIES);
}
