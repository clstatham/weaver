//! Removal detection (`RemovedComponents`), despawning, and replacing components.

mod common;

use std::sync::Mutex;

use common::init_task_pool;
use weaver_ecs::{change_detection::ChangeDetection, prelude::*};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
struct Stage1;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
struct Stage2;

#[derive(Debug, PartialEq)]
struct A(u32);
struct B;

struct ToRemove(Vec<Entity>);
struct ToDespawn(Vec<Entity>);

fn remove_and_despawn(
    commands: Commands,
    mut to_remove: ResMut<ToRemove>,
    mut to_despawn: ResMut<ToDespawn>,
) {
    for entity in to_remove.0.drain(..) {
        commands.remove_component::<A>(entity);
    }
    for entity in to_despawn.0.drain(..) {
        commands.despawn(entity);
    }
}

static SEEN: Mutex<Vec<Vec<Entity>>> = Mutex::new(Vec::new());

fn record_removed(removed: RemovedComponents<A>) {
    let mut seen: Vec<Entity> = removed.into_iter().collect();
    seen.sort_by_key(|e| e.as_u64());
    SEEN.lock().unwrap().push(seen);
}

/// Removals and despawns show up in a later system exactly once.
#[test]
fn removed_components_reports_each_removal_once() {
    init_task_pool();
    let mut world = World::new();
    let kept = world.spawn((A(0), B));
    let removed = world.spawn((A(1), B));
    let despawned = world.spawn((A(2),));
    let unrelated = world.spawn((B,));

    world.insert_resource(ToRemove(vec![removed]));
    world.insert_resource(ToDespawn(vec![despawned, unrelated]));
    world.push_update_stage(Stage1);
    world.push_update_stage(Stage2);
    world.add_system(remove_and_despawn, Stage1);
    world.add_system(record_removed, Stage2);
    world.initialize_systems();

    world.update().unwrap();
    world.update().unwrap();

    let mut expected = vec![removed, despawned];
    expected.sort_by_key(|e| e.as_u64());
    let seen = SEEN.lock().unwrap().clone();
    assert_eq!(seen, vec![expected, vec![]]);

    assert!(world.has_component::<A>(kept));
    assert!(!world.has_component::<A>(removed));
    assert!(world.has_component::<B>(removed));
    assert!(!world.is_alive(despawned));
    assert!(!world.is_alive(unrelated));
}

#[test]
fn removing_from_missing_entity_or_component_is_a_no_op() {
    let world = World::new();
    let entity = world.spawn((B,));
    assert_eq!(world.remove_component::<A>(entity), None);
    world.destroy_entity(entity);
    assert_eq!(world.remove_component::<A>(entity), None);
    // destroying twice is fine too
    world.destroy_entity(entity);
}

/// Inserting a component the entity already has replaces the value, and counts as a change but
/// not as an addition.
#[test]
fn inserting_an_existing_component_replaces_it() {
    let world = World::new();
    let entity = world.spawn((A(1), B));
    world.increment_change_tick();

    world.insert_component(entity, A(2));

    let mut query = world.query::<&A>();
    let a = query.get(entity).unwrap();
    assert_eq!(*a, A(2));
    assert!(a.is_changed());
    assert!(!a.is_added());
}
