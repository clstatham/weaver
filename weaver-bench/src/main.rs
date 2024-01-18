#![feature(test)]

extern crate test;

#[derive(Debug, Default, weaver_ecs::Component, bevy_ecs::component::Component)]
pub struct TestComponent {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const N: usize = 32_000;

fn main() {
    main_weaver()
}

fn main_weaver() {
    let mut world = weaver_ecs::World::new();

    for _ in 0..N {
        world.spawn((TestComponent::default(),));
    }

    loop {
        let q = world.query::<&TestComponent>();
        q.iter().count();
    }
}

fn main_hecs() {
    let mut world = hecs::World::new();

    for _ in 0..N {
        world.spawn((TestComponent::default(),));
    }

    loop {
        let mut q = world.query::<&TestComponent>();
        q.iter().count();
    }
}

#[bench]
fn bench_weaver_query(b: &mut test::Bencher) {
    let mut world = weaver_ecs::world::World::new();

    for _ in 0..N {
        world.spawn(TestComponent {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
    }

    b.iter(|| {
        test::black_box({
            let q = world.query::<&TestComponent>();
            q.iter().count()
        });
    });
}

#[bench]
fn bench_bevy_query(b: &mut test::Bencher) {
    let mut world = bevy_ecs::world::World::new();

    for _ in 0..N {
        world.spawn(TestComponent {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
    }

    b.iter(|| {
        test::black_box({
            let mut q = world.query::<&TestComponent>();
            q.iter(&world).count()
        });
    });
}

#[bench]
fn bench_hecs_query(b: &mut test::Bencher) {
    let mut world = hecs::World::new();

    for _ in 0..N {
        world.spawn((TestComponent {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },));
    }

    b.iter(|| {
        test::black_box({
            let mut q = world.query::<&TestComponent>();
            q.iter().count()
        });
    });
}
