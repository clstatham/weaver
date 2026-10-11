//! Entity identity.
//!
//! Tests marked `#[ignore = "known issue: ..."]` assert the intended behavior and currently fail.
//! Run them with `cargo test -p weaver-ecs -- --ignored`.

use weaver_ecs::prelude::*;

#[derive(Debug)]
struct A;
#[derive(Debug)]
struct B;

/// Inserting into an entity that isn't alive in this world (despawned, or allocated by another
/// world) is ignored, so it can't create a zombie entity or collide with a later spawn.
#[test]
fn inserting_into_a_dead_or_foreign_entity_is_ignored() {
    let main_world = World::new();
    let foreign = main_world.spawn((A,));

    let render_world = World::new();
    render_world.insert_component(foreign, A);
    assert!(render_world.query::<&A>().iter().next().is_none());
    let local = render_world.spawn((B,));
    assert!(render_world.has_component::<B>(local));

    render_world.destroy_entity(local);
    render_world.insert_component(local, A);
    assert!(render_world.query::<&A>().iter().next().is_none());
    assert!(!render_world.is_alive(local));
}
