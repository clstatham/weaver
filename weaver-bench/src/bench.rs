use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[derive(Debug, Default, weaver_ecs::Component)]
pub struct TestComponent {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const ENTITY_COUNTS: &[usize] = &[100, 1000, 10000, 100000];

pub fn bench_weaver_query(c: &mut Criterion) {
    let plot_config =
        criterion::PlotConfiguration::default().summary_scale(criterion::AxisScale::Logarithmic);

    let mut group = c.benchmark_group("weaver_query");
    group.plot_config(plot_config);
    group.sampling_mode(criterion::SamplingMode::Linear);

    for n in ENTITY_COUNTS.iter() {
        group.throughput(criterion::Throughput::Elements(*n as u64));
        group.bench_with_input(criterion::BenchmarkId::from_parameter(n), n, |b, n| {
            let mut world = weaver_ecs::world::World::new();

            for _ in 0..*n {
                world.spawn(TestComponent {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                });
            }

            b.iter(|| {
                let q = world.query::<&TestComponent>();
                black_box(q.iter().count());
            })
        });
    }

    group.finish();
}

criterion_group!(benches, bench_weaver_query);
criterion_main!(benches);
