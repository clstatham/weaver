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

pub const ENTITY_COUNTS: &[usize] = &[100, 1_000, 10_000, 100_000];

pub fn weaver_query_iter_many_entities(c: &mut Criterion) {
    use weaver_ecs::*;

    let plot_config =
        criterion::PlotConfiguration::default().summary_scale(criterion::AxisScale::Logarithmic);

    let mut group = c.benchmark_group("weaver_query_iter_many_entities");
    group.plot_config(plot_config);
    group.sampling_mode(criterion::SamplingMode::Linear);

    for n in ENTITY_COUNTS.iter() {
        group.throughput(criterion::Throughput::Elements(*n as u64));
        group.bench_with_input(criterion::BenchmarkId::from_parameter(n), n, |b, n| {
            let mut world = World::new();

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

pub fn weaver_query_par_iter_many_entities(c: &mut Criterion) {
    use weaver_ecs::*;

    let plot_config =
        criterion::PlotConfiguration::default().summary_scale(criterion::AxisScale::Logarithmic);

    let mut group = c.benchmark_group("weaver_query_par_iter_many_entities");
    group.plot_config(plot_config);
    group.sampling_mode(criterion::SamplingMode::Linear);

    for n in ENTITY_COUNTS.iter() {
        group.throughput(criterion::Throughput::Elements(*n as u64));
        group.bench_with_input(criterion::BenchmarkId::from_parameter(n), n, |b, n| {
            let mut world = World::new();

            for _ in 0..*n {
                world.spawn(A {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                });
            }

            b.iter(|| {
                let q = world.query::<&A>();
                black_box(q.par_iter().count());
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    weaver_query_iter_many_entities,
    weaver_query_par_iter_many_entities,
);
criterion_main!(benches);
