//! Commands issued while queries are alive.
//!
//! Kept in their own test binary: a deadlocked system permanently occupies a task pool worker,
//! which could starve other tests sharing the pool.
//!
//! Tests marked `#[ignore = "known issue: ..."]` assert the intended behavior and currently fail.
//! Run them with `cargo test -p weaver-ecs -- --ignored`.

mod common;

use common::{init_task_pool, with_timeout};
use weaver_ecs::prelude::*;

struct A(u32);
struct Marker;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
struct Stage;

fn mutate_then_spawn(mut query: Query<&mut A>, commands: Commands) {
    for mut a in query.iter() {
        a.0 += 1;
    }
    commands.spawn((A(100),));
}

/// Issuing a command must not require the issuing system to release its own queries first.
#[test]
fn command_while_holding_own_query_does_not_deadlock() {
    init_task_pool();
    let values = with_timeout(5, || {
        let mut world = World::new();
        world.spawn((A(0),));
        world.push_update_stage(Stage);
        world.add_system(mutate_then_spawn, Stage);
        world.initialize_systems();
        world.update().unwrap();
        let mut query = world.query::<&A>();
        let mut values = query.iter().map(|a| a.0).collect::<Vec<_>>();
        values.sort();
        values
    });
    let values = values.expect("World::update deadlocked");
    // a dropped command can't deadlock either, so check that it was applied
    assert_eq!(values, vec![1, 100]);
}

fn write_then_command(mut query: Query<&mut A>, commands: Commands) {
    for mut a in query.iter() {
        a.0 += 1;
    }
    commands.insert_resource(Marker);
}

fn read_only(mut query: Query<&A>) {
    for _ in query.iter() {}
}

/// Returns whether the writer's command was applied, or `None` if the update deadlocked.
fn run_writer_and_reader(writer_first: bool) -> Option<bool> {
    with_timeout(5, move || {
        let mut world = World::new();
        world.spawn((A(0),));
        world.push_update_stage(Stage);
        if writer_first {
            world.add_system(write_then_command, Stage);
            world.add_system(read_only, Stage);
        } else {
            world.add_system(read_only, Stage);
            world.add_system(write_then_command, Stage);
        }
        world.initialize_systems();
        world.update().unwrap();
        world.has_resource::<Marker>()
    })
}

/// A writer that issues any command and a reader of the same component, in one stage, must not
/// deadlock, regardless of the order they were registered in.
#[test]
fn writer_with_command_and_reader_do_not_deadlock() {
    init_task_pool();
    assert_eq!(
        run_writer_and_reader(false),
        Some(true),
        "deadlocked (None) or dropped the command (false) with the reader registered first"
    );
    assert_eq!(
        run_writer_and_reader(true),
        Some(true),
        "deadlocked (None) or dropped the command (false) with the writer registered first"
    );
}
