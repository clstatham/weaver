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

use weaver_ecs::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
struct Stage1;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
struct Stage2;

fn quick(_commands: Commands) {}

fn slow(_commands: Commands) {
    std::thread::sleep(Duration::from_millis(30));
}

/// The change tick should advance by a small, deterministic amount per update, regardless of how
/// long the systems take to run.
#[test]
fn change_tick_advance_per_update_is_bounded() {
    let mut world = World::new();
    world.push_update_stage(Stage1);
    world.add_system(quick, Stage1);
    world.add_system(slow, Stage1);
    world.initialize_systems();

    for _ in 0..3 {
        let before = world.read_change_tick().as_u64();
        world.update().unwrap();
        let advance = world.read_change_tick().as_u64() - before;
        assert!(
            advance <= 16,
            "one update advanced the change tick by {advance}"
        );
    }
}

static AFTER_PANIC_RAN: AtomicBool = AtomicBool::new(false);

fn panics(_commands: Commands) {
    panic!("system panicked on purpose");
}

fn runs_after_panic(_commands: Commands) {
    AFTER_PANIC_RAN.store(true, Ordering::SeqCst);
}

/// A panicking system must be reported, either by propagating the panic or by
/// `World::update` returning an error.
#[test]
fn system_panic_is_reported() {
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

fn first(_commands: Commands) {
    FIRST_RAN.store(true, Ordering::SeqCst);
}

fn second(_commands: Commands) {
    SECOND_SAW_FIRST.store(FIRST_RAN.load(Ordering::SeqCst), Ordering::SeqCst);
}

/// `.after(x)` should work even if `x` is registered later in the same stage.
#[test]
fn after_does_not_depend_on_registration_order() {
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

fn write_shared<const N: usize>(_shared: ResMut<Shared>) {
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
fn many_systems_writing_one_resource_can_be_scheduled() {
    let mut world = World::new();
    world.insert_resource(Shared);
    world.push_update_stage(Stage1);
    add_writers!(world, 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19);
    world.initialize_systems();
    world.update().unwrap();

    assert_eq!(SHARED_WRITES.load(Ordering::SeqCst), 20);
}

static DUPLICATE_RUNS: AtomicUsize = AtomicUsize::new(0);

fn counted(_commands: Commands) {
    DUPLICATE_RUNS.fetch_add(1, Ordering::SeqCst);
}

/// `World::add_system` documents that adding the same system to a stage twice is ignored.
#[test]
fn adding_a_system_twice_runs_it_once() {
    let mut world = World::new();
    world.push_update_stage(Stage1);
    world.add_system(counted, Stage1);
    world.add_system(counted, Stage1);
    world.initialize_systems();
    world.update().unwrap();

    assert_eq!(DUPLICATE_RUNS.load(Ordering::SeqCst), 1);
}

struct Counter(u32);

fn bump_counter(mut counter: ResMut<Counter>) {
    counter.0 += 1;
}

static WORLD_SYSTEM_SAW: AtomicUsize = AtomicUsize::new(0);

fn read_through_world(world: &World) {
    // a `&World` system can fetch any parameter on demand
    let mut state = SystemState::<Res<Counter>>::new(world);
    let counter = state.get(world).unwrap();
    WORLD_SYSTEM_SAW.store(counter.0 as usize, Ordering::SeqCst);
    assert!(SystemState::<Res<Shared>>::new(world).get(world).is_none());
}

#[test]
fn world_systems_run_in_order_and_can_fetch_params() {
    let mut world = World::new();
    world.insert_resource(Counter(0));
    world.push_update_stage(Stage1);
    world.add_system(read_through_world.after(bump_counter), Stage1);
    world.add_system(bump_counter, Stage1);
    world.initialize_systems();
    world.update().unwrap();
    world.update().unwrap();

    assert_eq!(WORLD_SYSTEM_SAW.load(Ordering::SeqCst), 2);
}
