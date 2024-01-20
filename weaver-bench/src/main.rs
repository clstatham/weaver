#![allow(unused)]

use weaver_ecs::prelude::*;

mod bench;

use bench::*;

fn main() {
    let mut world = weaver_ecs::world::World::new();

    for _ in 0..10_000 {
        world.spawn((A, B, C, D, E, F, G, H));
    }

    let q = world.query::<(&A, &B, &C, &D, &E, &F, &G, &H)>();
    loop {
        let count = q.iter().count();
        assert_eq!(count, 10_000);
    }
}
