//! Entity identity.
//!
//! Tests marked `#[ignore = "known issue: ..."]` assert the intended behavior and currently fail.
//! Run them with `cargo test -p weaver-ecs -- --ignored`.

use weaver_ecs::prelude::*;

#[derive(Debug)]
struct A;
#[derive(Debug)]
struct B;

/// The render world receives components for main-world entities via `insert_component`
/// without allocating them. A later `spawn` in that world must not hand out the same entity.
#[test]
#[ignore = "known issue: insert_component on an unallocated entity doesn't reserve it, so spawn reuses the id"]
fn spawn_does_not_reuse_an_entity_inserted_from_another_world() {
    let mut main_world = World::new();
    let mirrored = main_world.spawn((A,));

    let mut render_world = World::new();
    render_world.insert_component(mirrored, A);
    let local = render_world.spawn((B,));

    assert_ne!(local, mirrored);
    assert!(!render_world.has_component::<B>(mirrored));
}
