//! System scheduling and execution.
//!
//! Tests marked `#[ignore = "known issue: ..."]` assert the intended behavior and currently fail.
//! Run them with `cargo test -p weaver-ecs -- --ignored`.

mod common;

use std::{
    panic::AssertUnwindSafe,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Duration,
};

use common::init_task_pool;
use weaver_ecs::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
struct Stage1;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
struct Stage2;

async fn quick(_commands: Commands) {}

async fn slow(_commands: Commands) {
    std::thread::sleep(Duration::from_millis(30));
}

/// The change tick should advance by a small, deterministic amount per update. It currently
/// increments on every pass of the scheduler's busy-wait loop, so it depends on system duration.
#[test]
#[ignore = "known issue: the scheduler bumps the change tick on every busy-wait pass"]
fn change_tick_advance_per_update_is_bounded() {
    init_task_pool();
    let mut world = World::new();
    world.push_update_stage(Stage1);
    world.add_system(quick, Stage1);
    world.add_system(slow, Stage1);
    world.initialize_systems();

    for _ in 0..3 {
        let before = world.read_change_tick().as_u64();
        world.update().unwrap();
        let advance = world.read_change_tick().as_u64() - before;
        assert!(advance <= 16, "one update advanced the change tick by {advance}");
    }
}

static AFTER_PANIC_RAN: AtomicBool = AtomicBool::new(false);

async fn panics(_commands: Commands) {
    panic!("system panicked on purpose");
}

async fn runs_after_panic(_commands: Commands) {
    AFTER_PANIC_RAN.store(true, Ordering::SeqCst);
}

/// A panicking system must be reported, either by propagating the panic or by
/// `World::update` returning an error.
#[test]
#[ignore = "known issue: system panics are caught by the task pool and never reach World::update"]
fn system_panic_is_reported() {
    init_task_pool();
    let mut world = World::new();
    world.push_update_stage(Stage1);
    world.push_update_stage(Stage2);
    world.add_system(panics, Stage1);
    world.add_system(runs_after_panic, Stage2);
    world.initialize_systems();

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| world.update()));

    assert!(
        !matches!(result, Ok(Ok(()))),
        "World::update returned Ok(()) after a system panicked (later stage ran: {})",
        AFTER_PANIC_RAN.load(Ordering::SeqCst)
    );
}

static FIRST_RAN: AtomicBool = AtomicBool::new(false);
static SECOND_SAW_FIRST: AtomicBool = AtomicBool::new(false);

async fn first(_commands: Commands) {
    FIRST_RAN.store(true, Ordering::SeqCst);
}

async fn second(_commands: Commands) {
    SECOND_SAW_FIRST.store(FIRST_RAN.load(Ordering::SeqCst), Ordering::SeqCst);
}

/// `.after(x)` should work even if `x` is registered later in the same stage.
#[test]
#[ignore = "known issue: .after(x) looks x up immediately and panics if it isn't registered yet"]
fn after_does_not_depend_on_registration_order() {
    init_task_pool();
    let mut world = World::new();
    world.push_update_stage(Stage1);
    world.add_system(second.after(first), Stage1);
    world.add_system(first, Stage1);
    world.initialize_systems();
    world.update().unwrap();

    assert!(SECOND_SAW_FIRST.load(Ordering::SeqCst));
}

struct Shared;
static SHARED_WRITES: AtomicUsize = AtomicUsize::new(0);

async fn write_shared<const N: usize>(_shared: ResMut<Shared>) {
    SHARED_WRITES.fetch_add(1, Ordering::SeqCst);
}

macro_rules! add_writers {
    ($world:expr, $($n:literal)*) => {
        $( $world.add_system(write_shared::<$n>, Stage1); )*
    };
}

/// Any number of systems writing the same resource should be serialized. The resolver gives up
/// after 100 added edges and reports a cycle, which a stage reaches at about 15 such systems.
#[test]
#[ignore = "known issue: dependency resolution is capped at 100 edges and misreports the failure as a cycle"]
fn many_systems_writing_one_resource_can_be_scheduled() {
    init_task_pool();
    let mut world = World::new();
    world.insert_resource(Shared);
    world.push_update_stage(Stage1);
    add_writers!(world, 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19);
    world.initialize_systems();
    world.update().unwrap();

    assert_eq!(SHARED_WRITES.load(Ordering::SeqCst), 20);
}

static DUPLICATE_RUNS: AtomicUsize = AtomicUsize::new(0);

async fn counted(_commands: Commands) {
    DUPLICATE_RUNS.fetch_add(1, Ordering::SeqCst);
}

/// `World::add_system` documents that adding the same system to a stage twice is ignored.
#[test]
#[ignore = "known issue: add_system doesn't check for duplicates despite its doc comment"]
fn adding_a_system_twice_runs_it_once() {
    init_task_pool();
    let mut world = World::new();
    world.push_update_stage(Stage1);
    world.add_system(counted, Stage1);
    world.add_system(counted, Stage1);
    world.initialize_systems();
    world.update().unwrap();

    assert_eq!(DUPLICATE_RUNS.load(Ordering::SeqCst), 1);
}
