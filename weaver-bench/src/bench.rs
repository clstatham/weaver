use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[derive(Debug, Default, weaver_ecs::Component)]
pub struct A {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Default, weaver_ecs::Component)]
pub struct B {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Default, weaver_ecs::Component)]
pub struct C {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Default, weaver_ecs::Component)]
pub struct D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const ENTITY_COUNTS: &[usize] = &[32, 512, 1024, 2048, 4096, 8192, 16384, 32768];

pub fn weaver_query_iter_many_entities(c: &mut Criterion) {
    let plot_config =
        criterion::PlotConfiguration::default().summary_scale(criterion::AxisScale::Logarithmic);

    let mut group = c.benchmark_group("weaver_query_iter_many_entities");
    group.plot_config(plot_config);
    group.sampling_mode(criterion::SamplingMode::Linear);

    for n in ENTITY_COUNTS.iter() {
        group.throughput(criterion::Throughput::Elements(*n as u64));
        group.bench_with_input(criterion::BenchmarkId::from_parameter(n), n, |b, n| {
            let mut world = weaver_ecs::world::World::new();

            for _ in 0..*n {
                world.spawn(A {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                });
            }

            b.iter(|| {
                let q = world.query::<&A>();
                black_box(q.iter().count());
            })
        });
    }

    group.finish();
}

pub fn weaver_query_get_last_entity(c: &mut Criterion) {
    let plot_config =
        criterion::PlotConfiguration::default().summary_scale(criterion::AxisScale::Logarithmic);

    let mut group = c.benchmark_group("weaver_query_get_last_entity");
    group.plot_config(plot_config);
    group.sampling_mode(criterion::SamplingMode::Linear);

    for n in ENTITY_COUNTS.iter() {
        group.throughput(criterion::Throughput::Elements(*n as u64));
        group.bench_with_input(criterion::BenchmarkId::from_parameter(n), n, |b, n| {
            let mut world = weaver_ecs::world::World::new();

            for _ in 0..*n {
                world.spawn(A {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                });
            }

            b.iter(|| {
                let q = world.query::<&A>();
                black_box(q.get((n - 1) as u32));
            })
        });
    }

    group.finish();
}

pub fn weaver_query_iter_many_archetypes(c: &mut Criterion) {
    let plot_config =
        criterion::PlotConfiguration::default().summary_scale(criterion::AxisScale::Logarithmic);

    let mut group = c.benchmark_group("weaver_query_iter_many_archetypes");
    group.plot_config(plot_config);
    group.sampling_mode(criterion::SamplingMode::Linear);

    for n in ENTITY_COUNTS.iter() {
        group.throughput(criterion::Throughput::Elements(*n as u64));
        group.bench_with_input(criterion::BenchmarkId::from_parameter(n), n, |b, n| {
            let mut world = weaver_ecs::world::World::new();

            for _ in 0..*n {
                world.spawn(A {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                });
            }

            for _ in 0..*n {
                world.spawn(B {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                });
            }

            for _ in 0..*n {
                world.spawn(C {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                });
            }

            for _ in 0..*n {
                world.spawn(D {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                });
            }

            for _ in 0..*n {
                world.spawn((
                    B {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    C {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    D {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ));
            }

            b.iter(|| {
                let q = world.query::<&A>();
                black_box(q.iter().count());
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    weaver_query_iter_many_entities,
    weaver_query_get_last_entity,
    weaver_query_iter_many_archetypes,
);
criterion_main!(benches);
