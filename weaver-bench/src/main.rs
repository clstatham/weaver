#![feature(test)]

extern crate test;

use std::sync::Arc;
use weaver::prelude::*;
use weaver_ecs::system::SystemStage;

#[system(Test)]
fn test(query: Query<&Transform>) {
    for transform in query.iter() {
        assert_eq!(transform.get_translation(), Vec3::ZERO);
    }
}

fn main() {
    let mut world = World::new();

    for _ in 0..32_000 {
        world.spawn((Transform::default(),)).unwrap();
    }

    let world = Arc::new(parking_lot::RwLock::new(world));

    loop {
        let _ = Query::<&Transform>::new(world.read().components());
    }
}

#[bench]
fn bench_query(b: &mut test::Bencher) {
    let mut world = World::new();

    for _ in 0..32_000 {
        world.spawn((Transform::default(),)).unwrap();
    }

    let world = Arc::new(parking_lot::RwLock::new(world));
    b.iter(|| {
        test::black_box({
            let _ = Query::<&Transform>::new(world.read().components());
        })
    });
}
