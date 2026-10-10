//! Query iteration and lookup.
//!
//! Tests marked `#[ignore = "known issue: ..."]` assert the intended behavior and currently fail.
//! Run them with `cargo test -p weaver-ecs -- --ignored`.

mod common;

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    time::{Duration, Instant},
};

use common::with_timeout;
use weaver_ecs::{change_detection::ChangeDetectionMut, prelude::*};

#[derive(Debug, PartialEq)]
struct A(u32);
struct B;
struct C;

/// A row-filtering element (`Changed<>`) in a tuple query must not shift the other elements:
/// every yielded `Entity` has to be paired with that entity's own data.
#[test]
fn changed_filter_in_tuple_keeps_rows_aligned() {
    let world = World::new();
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
fn optional_only_query_yields_one_item_per_entity() {
    let world = World::new();
    world.spawn((A(0),));
    world.spawn((C,));

    let single = world.query::<Option<&B>>().iter().take(1000).count();
    assert_eq!(single, 2);

    let pair = world
        .query::<(Option<&A>, Option<&B>)>()
        .iter()
        .take(1000)
        .count();
    assert_eq!(pair, 2);
}

fn min_query_build_time(entities: u32) -> Duration {
    let world = World::new();
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
fn query_construction_scales_linearly() {
    let small = min_query_build_time(2_000);
    let large = min_query_build_time(8_000);
    let ratio = large.as_secs_f64() / small.as_secs_f64();

    assert!(
        ratio < 8.0,
        "4x the entities took {ratio:.1}x as long to build a query ({small:?} -> {large:?})"
    );
}

/// `get` on a tuple with an `Option` element must return `Some((.., None))` for an entity
/// whose archetype lacks the optional component, not `None`.
#[test]
fn get_with_missing_optional_component_returns_none_element() {
    let world = World::new();
    let entity = world.spawn((A(1),));

    let mut query = world.query::<(Entity, Option<&B>)>();
    let item = query.get(entity).map(|(e, b)| (e, b.is_none()));
    assert_eq!(item, Some((entity, true)));
}

/// A query whose own terms would alias a component must panic up front instead of deadlocking.
#[test]
fn aliasing_terms_in_one_query_panic() {
    let panicked = with_timeout(5, || {
        let world = World::new();
        world.spawn((A(0),));
        let both = catch_unwind(AssertUnwindSafe(|| drop(world.query::<(&mut A, &A)>())));
        let changed = catch_unwind(AssertUnwindSafe(|| {
            drop(world.query::<(&mut A, Changed<&A>)>())
        }));
        both.is_err() && changed.is_err()
    });
    assert_eq!(panicked, Some(true));
}

/// Two live queries with conflicting access to a component must panic, and a failed query must
/// release whatever it had already borrowed.
#[test]
fn conflicting_live_queries_panic_and_release_borrows() {
    let world = World::new();
    world.spawn((A(0), B));

    {
        let _writer = world.query::<&mut A>();
        assert!(catch_unwind(AssertUnwindSafe(|| drop(world.query::<&mut A>()))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| drop(world.query::<&A>()))).is_err());
    }

    {
        let _first = world.query::<&A>();
        let _second = world.query::<&A>();
    }

    {
        // the tuple borrows A, then fails on B; A must be released again
        let _b_reader = world.query::<&B>();
        assert!(
            catch_unwind(AssertUnwindSafe(|| drop(world.query::<(&mut A, &mut B)>()))).is_err()
        );
        let mut a = world.query::<&mut A>();
        assert_eq!(a.iter().count(), 1);
    }
}

/// Structural changes need exclusive access to storage, so they panic while a query is alive,
/// and work again once it's dropped.
#[test]
fn structural_change_while_query_alive_panics() {
    let world = World::new();
    world.spawn((A(0),));

    let query = world.query::<&A>();
    assert!(catch_unwind(AssertUnwindSafe(|| world.spawn((A(1),)))).is_err());
    drop(query);

    world.spawn((A(2),));
    assert_eq!(world.query::<&A>().iter().count(), 2);
}

#[test]
fn without_filters_out_archetypes_with_the_component() {
    let world = World::new();
    world.spawn((A(1),));
    world.spawn((A(2), B));
    world.spawn((A(3), C));

    let mut query = world.query::<(&A, Without<B>)>();
    let mut values: Vec<u32> = query.iter().map(|(a, _)| a.0).collect();
    values.sort();
    assert_eq!(values, vec![1, 3]);
}

/// Destroying an entity moves the archetype's last row into the hole, and lookups must follow it.
#[test]
fn get_finds_entities_after_rows_move() {
    let world = World::new();
    let entities: Vec<Entity> = (0..5).map(|i| world.spawn((A(i),))).collect();
    world.destroy_entity(entities[1]);
    world.destroy_entity(entities[0]);

    let mut query = world.query::<&A>();
    assert!(query.get(entities[0]).is_none());
    assert!(query.get(entities[1]).is_none());
    for (i, entity) in entities.iter().enumerate().skip(2) {
        assert_eq!(query.get(*entity).unwrap().0, i as u32, "entity {i}");
    }
}

/// Mutates `T` on a fresh `(A, B, C)` entity, removes `T`, and returns how many of the
/// remaining components report a change.
fn changed_after_removing<T: Component>() -> usize {
    let world = World::new();
    let entity = world.spawn((A(0), B, C));
    world.increment_change_tick();
    world.query::<&mut T>().get(entity).unwrap().set_changed();
    world.remove_component::<T>(entity);

    world.query::<Changed<&A>>().iter().count()
        + world.query::<Changed<&B>>().iter().count()
        + world.query::<Changed<&C>>().iter().count()
}

/// Removing a component must drop its ticks too, or the ticks of the remaining columns shift.
#[test]
fn removing_a_component_keeps_ticks_aligned() {
    assert_eq!(changed_after_removing::<A>(), 0);
    assert_eq!(changed_after_removing::<B>(), 0);
    assert_eq!(changed_after_removing::<C>(), 0);
}

fn min_get_all_time(entities: u32) -> Duration {
    let world = World::new();
    let spawned: Vec<Entity> = (0..entities).map(|i| world.spawn((A(i),))).collect();
    let mut query = world.query::<&A>();
    (0..3)
        .map(|_| {
            let start = Instant::now();
            for entity in &spawned {
                std::hint::black_box(query.get(*entity));
            }
            start.elapsed()
        })
        .min()
        .unwrap()
}

/// `Query::get` should be O(1), so looking up every entity is O(entities). Timing-based.
#[test]
fn get_scales_linearly() {
    let small = min_get_all_time(2_000);
    let large = min_get_all_time(8_000);
    let ratio = large.as_secs_f64() / small.as_secs_f64();

    assert!(
        ratio < 8.0,
        "4x the entities took {ratio:.1}x as long to look up ({small:?} -> {large:?})"
    );
}
