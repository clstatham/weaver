//! Deferred `Commands`: when they're applied, and the entities they reserve.

mod common;

use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use common::init_task_pool;
use weaver_ecs::prelude::*;

struct A(u32);
struct B;
struct Marker;

#[derive(Default)]
struct Seen(AtomicBool);

#[derive(Default)]
struct Spawned(Mutex<Vec<Entity>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
struct Stage;

fn insert_marker(commands: Commands) {
    commands.insert_resource(Marker);
}

fn record_marker(marker: Option<Res<Marker>>, seen: Res<Seen>) {
    seen.0.store(marker.is_some(), Ordering::SeqCst);
}

fn needs_marker(_marker: Res<Marker>, seen: Res<Seen>) {
    seen.0.store(true, Ordering::SeqCst);
}

fn spawn_two(commands: Commands, spawned: Res<Spawned>) {
    let first = commands.spawn((A(1),));
    let second = commands.spawn((A(2),));
    spawned.0.lock().unwrap().extend([first, second]);
}

fn seen(world: &World) -> bool {
    world
        .get_resource::<Seen>()
        .unwrap()
        .0
        .load(Ordering::SeqCst)
}

/// Commands are applied when their layer finishes, so a system ordered after the issuer sees them
/// in the same update.
#[test]
fn later_layers_see_commands_from_earlier_layers() {
    init_task_pool();
    let mut world = World::new();
    world.insert_resource(Seen::default());
    world.push_update_stage(Stage);
    world.add_system(insert_marker, Stage);
    world.add_system(record_marker.after(insert_marker), Stage);
    world.initialize_systems();
    world.update().unwrap();

    assert!(seen(&world));
}

/// A system skipped for a missing resource is retried at the end of its layer. The renderer relies
/// on that as an implicit run condition, so the retry must see resources inserted by the layer's
/// commands.
#[test]
fn skipped_system_sees_resource_inserted_by_same_layer_command() {
    init_task_pool();
    let mut world = World::new();
    world.insert_resource(Seen::default());
    world.push_update_stage(Stage);
    world.add_system(insert_marker, Stage);
    world.add_system(needs_marker, Stage);
    world.initialize_systems();
    world.update().unwrap();

    assert!(seen(&world));
}

/// `Commands::spawn` returns its entity before the command is applied. Once applied, the entity has
/// its components, and a later `World::spawn` doesn't reuse its id.
#[test]
fn spawned_entities_get_their_components_and_are_not_reused() {
    init_task_pool();
    let mut world = World::new();
    world.insert_resource(Spawned::default());
    world.push_update_stage(Stage);
    world.add_system(spawn_two, Stage);
    world.initialize_systems();
    world.update().unwrap();

    let spawned = world
        .get_resource::<Spawned>()
        .unwrap()
        .0
        .lock()
        .unwrap()
        .clone();
    assert_eq!(spawned.len(), 2);
    assert_ne!(spawned[0], spawned[1]);

    let local = world.spawn((B,));
    assert!(!spawned.contains(&local));

    let mut query = world.query::<&A>();
    assert_eq!(query.get(spawned[0]).map(|a| a.0), Some(1));
    assert_eq!(query.get(spawned[1]).map(|a| a.0), Some(2));
}

/// Between `Commands::spawn` and the apply, the reserved entity resolves by id, and a direct
/// `World::spawn` doesn't hand out the same id.
#[test]
fn reserved_entities_resolve_and_are_not_reused_before_flush() {
    let world = World::new();
    let reserved = world.entities_handle().read().reserve();

    assert_eq!(world.find_entity_by_id(reserved.id()), Some(reserved));
    let local = world.spawn((B,));
    assert_ne!(local, reserved);
}
