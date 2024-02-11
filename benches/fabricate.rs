use criterion::{black_box, criterion_group, criterion_main, Criterion};
use weaver::fabricate::prelude::*;

#[derive(Component, Clone)]
pub struct A {
    pub a: i32,
}

#[derive(Component, Clone)]
pub struct B {
    pub b: i32,
}

#[derive(Component, Clone)]
pub struct C {
    pub c: i32,
}

#[derive(Component, Clone)]
pub struct D {
    pub d: i32,
}

const ENTITY_COUNTS: &[usize] = &[1, 100, 10_000];

pub fn bench_query_1_component(c: &mut Criterion) {
    let mut group = c.benchmark_group("bench_query_1_component");
    group.plot_config(
        criterion::PlotConfiguration::default().summary_scale(criterion::AxisScale::Logarithmic),
    );

    for n in ENTITY_COUNTS {
        let world = World::new_handle();
        for _ in 0..*n {
            world.spawn((A { a: 0 },)).unwrap();
        }

        group.throughput(criterion::Throughput::Elements(*n as u64));
        group.bench_function(n.to_string().as_str(), |b| {
            b.iter(|| {
                world.query(
                    |q| q.read::<A>().unwrap(),
                    |query| {
                        for results in query.iter() {
                            black_box(results);
                        }
                    },
                );
            })
        });
    }

    group.finish();
}

pub fn bench_query_2_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("bench_query_2_components");
    group.plot_config(
        criterion::PlotConfiguration::default().summary_scale(criterion::AxisScale::Logarithmic),
    );
    group.throughput(criterion::Throughput::Elements(2));
    for n in ENTITY_COUNTS {
        let world = World::new_handle();
        for _ in 0..*n {
            world.spawn((A { a: 0 }, B { b: 0 })).unwrap();
        }

        group.throughput(criterion::Throughput::Elements(2 * *n as u64));
        group.bench_function(n.to_string().as_str(), |b| {
            b.iter(|| {
                world.query(
                    |q| q.read::<A>().unwrap().read::<B>().unwrap(),
                    |query| {
                        for results in query.iter() {
                            black_box(results);
                        }
                    },
                );
            })
        });
    }

    group.finish();
}

pub fn bench_query_4_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("bench_query_4_components");
    group.plot_config(
        criterion::PlotConfiguration::default().summary_scale(criterion::AxisScale::Logarithmic),
    );
    for n in ENTITY_COUNTS {
        let world = World::new_handle();
        for _ in 0..*n {
            world
                .spawn((A { a: 0 }, B { b: 0 }, C { c: 0 }, D { d: 0 }))
                .unwrap();
        }

        group.throughput(criterion::Throughput::Elements(4 * *n as u64));
        group.bench_function(n.to_string().as_str(), |b| {
            b.iter(|| {
                world.query(
                    |q| {
                        q.read::<A>()
                            .unwrap()
                            .read::<B>()
                            .unwrap()
                            .read::<C>()
                            .unwrap()
                            .read::<D>()
                            .unwrap()
                    },
                    |query| {
                        for results in query.iter() {
                            black_box(results);
                        }
                    },
                );
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_query_1_component,
    bench_query_2_components,
    bench_query_4_components
);
criterion_main!(benches);
