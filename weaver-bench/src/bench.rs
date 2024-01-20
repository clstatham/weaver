use weaver_ecs::prelude::*;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

macro_rules! components {
    ($($name:ident),*) => {
        $(
            #[derive(Debug, Default, Component, Clone, Copy)]
            pub struct $name;
        )*
    }
}

components!(A, B, C, D, E, F, G, H);

pub const ENTITY_COUNTS: &[usize] = &[1, 10, 100, 1_000, 10_000];
pub const ENTITY_DEFAULT_COUNT: usize = 10_000;
pub const COMPONENT_COUNTS: &[usize] = &[1, 2, 4, 8];
pub const ARCHETYPE_COUNTS: &[usize] = &[1, 2, 4, 8];

pub fn weaver_query_iter_increasing_entities(c: &mut Criterion) {
    let plot_config =
        criterion::PlotConfiguration::default().summary_scale(criterion::AxisScale::Logarithmic);

    let mut group = c.benchmark_group("weaver_query_iter_increasing_entities");
    group.plot_config(plot_config);
    group.sampling_mode(criterion::SamplingMode::Linear);

    for &n in ENTITY_COUNTS.iter() {
        let mut world = World::new();

        for _ in 0..n {
            world.spawn(A);
        }

        group.throughput(criterion::Throughput::Elements(n as u64));
        group.bench_with_input(format!("{} entities", n), &n, |b, _| {
            let q = world.query::<&A>();
            b.iter(|| {
                black_box(q.iter().count());
            })
        });
    }

    group.finish();
}

pub fn weaver_query_iter_increasing_components(c: &mut Criterion) {
    let plot_config =
        criterion::PlotConfiguration::default().summary_scale(criterion::AxisScale::Logarithmic);

    let mut group = c.benchmark_group("weaver_query_iter_increasing_components");
    group.plot_config(plot_config);
    group.sampling_mode(criterion::SamplingMode::Linear);

    for n in COMPONENT_COUNTS.iter() {
        let mut world = World::new();

        match n {
            1 => {
                for _ in 0..ENTITY_DEFAULT_COUNT {
                    world.spawn(A);
                }

                let q = world.query::<&A>();
                group.throughput(criterion::Throughput::Elements(*n as u64));
                group.bench_with_input(format!("{} components", n), n, |b, _| {
                    b.iter(|| {
                        black_box(q.iter().count());
                    })
                });
            }
            2 => {
                for _ in 0..ENTITY_DEFAULT_COUNT {
                    world.spawn((A, B));
                }

                let q = world.query::<(&A, &B)>();
                group.throughput(criterion::Throughput::Elements(*n as u64));
                group.bench_with_input(format!("{} components", n), n, |b, _| {
                    b.iter(|| {
                        black_box(q.iter().count());
                    })
                });
            }
            4 => {
                for _ in 0..ENTITY_DEFAULT_COUNT {
                    world.spawn((A, B, C, D));
                }

                let q = world.query::<(&A, &B, &C, &D)>();
                group.throughput(criterion::Throughput::Elements(*n as u64));
                group.bench_with_input(format!("{} components", n), n, |b, _| {
                    b.iter(|| {
                        black_box(q.iter().count());
                    })
                });
            }
            8 => {
                for _ in 0..ENTITY_DEFAULT_COUNT {
                    world.spawn((A, B, C, D, E, F, G, H));
                }

                let q = world.query::<(&A, &B, &C, &D, &E, &F, &G, &H)>();
                group.throughput(criterion::Throughput::Elements(*n as u64));
                group.bench_with_input(format!("{} components", n), n, |b, _| {
                    b.iter(|| {
                        black_box(q.iter().count());
                    })
                });
            }
            _ => unreachable!(),
        }
    }

    group.finish();
}

pub fn weaver_query_iter_increasing_archetypes(c: &mut Criterion) {
    let plot_config =
        criterion::PlotConfiguration::default().summary_scale(criterion::AxisScale::Logarithmic);

    let mut group = c.benchmark_group("weaver_query_iter_increasing_archetypes");
    group.plot_config(plot_config);
    group.sampling_mode(criterion::SamplingMode::Linear);

    for n in ARCHETYPE_COUNTS.iter() {
        let mut world = World::new();

        match n {
            1 => {
                for _ in 0..ENTITY_DEFAULT_COUNT {
                    world.spawn(A);
                }
            }
            2 => {
                for _ in 0..ENTITY_DEFAULT_COUNT {
                    world.spawn(A);
                    world.spawn(B);
                }
            }
            4 => {
                for _ in 0..ENTITY_DEFAULT_COUNT {
                    world.spawn(A);
                    world.spawn(B);
                    world.spawn(C);
                    world.spawn(D);
                }
            }
            8 => {
                for _ in 0..ENTITY_DEFAULT_COUNT {
                    world.spawn(A);
                    world.spawn(B);
                    world.spawn(C);
                    world.spawn(D);
                    world.spawn(E);
                    world.spawn(F);
                    world.spawn(G);
                    world.spawn(H);
                }
            }
            _ => unreachable!(),
        }

        let q = world.query::<&A>();
        group.throughput(criterion::Throughput::Elements(*n as u64));
        group.bench_with_input(format!("{} archetypes", n), n, |b, _| {
            b.iter(|| {
                black_box(q.iter().count());
            })
        });
    }

    group.finish();
}

criterion_group!(
    weaver_benches,
    weaver_query_iter_increasing_entities,
    weaver_query_iter_increasing_components,
    weaver_query_iter_increasing_archetypes
);
criterion_main!(weaver_benches);
