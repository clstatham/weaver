#![feature(test)]

extern crate test;

use std::sync::Arc;
use weaver::ecs::system::SystemStage;
use weaver::prelude::*;

#[system(Test)]
fn test(query: Query<&Transform>) {
    for transform in query.iter() {
        assert_eq!(transform.get_translation(), Vec3::ZERO);
    }
}

#[bench]
fn bench_ecs(b: &mut test::Bencher) {
    let mut world = World::new();

    for _ in 0..32_000 {
        world.spawn((Transform::default(),)).unwrap();
    }

    world.add_system_to_stage(Test, SystemStage::Update);

    let world = Arc::new(parking_lot::RwLock::new(world));
    b.iter(|| {
        World::run_stage(&world, SystemStage::Update).unwrap();
    });
}
