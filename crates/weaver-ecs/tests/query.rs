//! Query iteration and lookup.
//!
//! Tests marked `#[ignore = "known issue: ..."]` assert the intended behavior and currently fail.
//! Run them with `cargo test -p weaver-ecs -- --ignored`.

use std::time::{Duration, Instant};

use weaver_ecs::prelude::*;

#[derive(Debug, PartialEq)]
struct A(u32);
struct B;
struct C;

/// A row-filtering element (`Changed<>`) in a tuple query must not shift the other elements:
/// every yielded `Entity` has to be paired with that entity's own data.
#[test]
#[ignore = "known issue: tuple queries zip per-column iterators, so filtered rows misalign"]
fn changed_filter_in_tuple_keeps_rows_aligned() {
    let mut world = World::new();
    let entities: Vec<Entity> = (0..5).map(|i| world.spawn((A(i),))).collect();
    world.increment_change_tick();
    {
        let mut query = world.query::<&mut A>();
        query.get(entities[3]).unwrap().0 = 333;
    }

    let mut query = world.query::<(Entity, Changed<&A>)>();
    let yielded: Vec<(Entity, u32)> = query.iter().map(|(e, a)| (e, a.0)).collect();

    assert_eq!(yielded, vec![(entities[3], 333)]);
}

/// A query made only of `Option<&T>` elements must yield exactly one item per entity,
/// including for archetypes that contain none of the optional components.
#[test]
#[ignore = "known issue: Option<&T> over an archetype without T is an endless repeat_with"]
fn optional_only_query_yields_one_item_per_entity() {
    let mut world = World::new();
    world.spawn((A(0),));
    world.spawn((C,));

    let single = world.query::<Option<&B>>().iter().take(1000).count();
    assert_eq!(single, 2);

    let pair = world.query::<(Option<&A>, Option<&B>)>().iter().take(1000).count();
    assert_eq!(pair, 2);
}

fn min_query_build_time(entities: u32) -> Duration {
    let mut world = World::new();
    for i in 0..entities {
        world.spawn((A(i),));
    }
    (0..3)
        .map(|_| {
            let start = Instant::now();
            let query = world.query::<&A>();
            let elapsed = start.elapsed();
            drop(query);
            elapsed
        })
        .min()
        .unwrap()
}

/// Building a query should cost O(entities). Quadrupling the entity count should roughly
/// quadruple the time; O(n^2) construction makes it ~16x. Timing-based, so it allows slack.
#[test]
#[ignore = "known issue: Query::new looks up each entity's row with a linear scan (O(n^2)); timing-based"]
fn query_construction_scales_linearly() {
    let small = min_query_build_time(2_000);
    let large = min_query_build_time(8_000);
    let ratio = large.as_secs_f64() / small.as_secs_f64();

    assert!(
        ratio < 8.0,
        "4x the entities took {ratio:.1}x as long to build a query ({small:?} -> {large:?})"
    );
}
