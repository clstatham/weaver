#![allow(unused)]

use weaver_ecs::prelude::*;
#[cfg(test)]
mod bench;

#[derive(Debug, Default, Component)]
pub struct TestComponent {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

fn main() {
    let mut world = weaver_ecs::world::World::new();

    for _ in 0..100_000 {
        world.spawn(TestComponent {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
    }

    loop {
        let q = world.query::<&TestComponent>();
        let count = q.iter().count();
        assert_eq!(count, 100_000);
    }
}
