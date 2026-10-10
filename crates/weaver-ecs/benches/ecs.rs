//! ECS storage and query benchmarks.
//!
//! These only use the public API, so the same file can measure the code before and after
//! internal reworks:
//!
//! ```sh
//! cargo bench -p weaver-ecs --bench ecs -- --save-baseline before
//! # ...change things...
//! cargo bench -p weaver-ecs --bench ecs -- --baseline before
//! ```

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use weaver_ecs::prelude::*;
use weaver_task::{task_pool::TaskPool, usages::GlobalTaskPool};

const SIZES: [u64; 3] = [100, 1_000, 10_000];

#[derive(Clone, Copy)]
struct Pos([f32; 3]);
#[derive(Clone, Copy)]
struct Vel([f32; 3]);
struct A(u64);
struct B(u64);
/// Zero-sized marker used to split entities over many archetypes.
struct M<const I: usize>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
struct BenchStage;

fn world_with_a(n: u64) -> (World, Vec<Entity>) {
    let world = World::new();
    let entities = (0..n).map(|i| world.spawn((A(i),))).collect();
    (world, entities)
}

/// Spreads `n` entities with an `A` over 20 archetypes.
fn fragmented_world(n: u64) -> World {
    macro_rules! spawn_in {
        ($world:ident, $i:ident, $($idx:literal)*) => {
            match $i % 20 {
                $($idx => { $world.spawn((A($i), M::<$idx>)); })*
                _ => unreachable!(),
            }
        };
    }
    let world = World::new();
    for i in 0..n {
        spawn_in!(world, i, 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19);
    }
    world
}

fn physics_world(n: u64) -> World {
    let world = World::new();
    for i in 0..n {
        let f = i as f32;
        world.spawn((Pos([f, f, f]), Vel([1.0, 0.5, 0.25])));
    }
    world
}

fn query_new(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_new");
    group.sample_size(10);
    for n in SIZES {
        group.throughput(Throughput::Elements(n));
        let (world, _) = world_with_a(n);
        group.bench_with_input(BenchmarkId::new("1arch", n), &n, |b, _| {
            b.iter(|| black_box(world.query::<&A>()));
        });
        let world = fragmented_world(n);
        group.bench_with_input(BenchmarkId::new("20arch", n), &n, |b, _| {
            b.iter(|| black_box(world.query::<&A>()));
        });
    }
}

fn iter_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_read");
    for n in SIZES {
        group.throughput(Throughput::Elements(n));
        let (world, _) = world_with_a(n);
        let mut query = world.query::<&A>();
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(query.iter().map(|a| a.0).sum::<u64>()));
        });
    }
}

fn iter_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_write");
    for n in SIZES {
        group.throughput(Throughput::Elements(n));
        let (world, _) = world_with_a(n);
        let mut query = world.query::<&mut A>();
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                for mut a in query.iter() {
                    a.0 = a.0.wrapping_add(1);
                }
            });
        });
    }
}

fn iter_tuple(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_tuple");
    for n in SIZES {
        group.throughput(Throughput::Elements(n));
        let world = physics_world(n);
        let mut query = world.query::<(Entity, &Pos, &mut Vel)>();
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                for (entity, pos, mut vel) in query.iter() {
                    black_box(entity);
                    for i in 0..3 {
                        vel.0[i] += pos.0[i] * 0.01;
                    }
                }
            });
        });
    }
}

fn iter_optional(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_optional");
    for n in SIZES {
        group.throughput(Throughput::Elements(n));
        let world = World::new();
        for i in 0..n {
            if i % 2 == 0 {
                world.spawn((A(i),));
            } else {
                world.spawn((A(i), B(i)));
            }
        }
        let mut query = world.query::<(&A, Option<&B>)>();
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                black_box(
                    query
                        .iter()
                        .map(|(a, b)| a.0 + b.map_or(0, |b| b.0))
                        .sum::<u64>(),
                )
            });
        });
    }
}

fn iter_fragmented(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_fragmented");
    for n in SIZES {
        group.throughput(Throughput::Elements(n));
        let world = fragmented_world(n);
        let mut query = world.query::<&mut A>();
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                for mut a in query.iter() {
                    a.0 = a.0.wrapping_add(1);
                }
            });
        });
    }
}

fn changed(c: &mut Criterion) {
    let mut group = c.benchmark_group("changed");
    for n in SIZES {
        group.throughput(Throughput::Elements(n));
        let (world, entities) = world_with_a(n);
        world.increment_change_tick();
        {
            let mut query = world.query::<&mut A>();
            for entity in entities.iter().step_by(10) {
                query.get(*entity).unwrap().0 += 1;
            }
        }
        let mut query = world.query::<Changed<&A>>();
        assert_eq!(query.iter().count() as u64, n.div_ceil(10));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(query.iter().count()));
        });
    }
}

fn get_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_all");
    group.sample_size(10);
    for n in SIZES {
        group.throughput(Throughput::Elements(n));
        let (world, entities) = world_with_a(n);
        let mut query = world.query::<&A>();
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let mut sum = 0;
                for entity in &entities {
                    sum += query.get(*entity).unwrap().0;
                }
                black_box(sum)
            });
        });
    }
}

fn integrate(mut query: Query<(&mut Pos, &Vel)>) {
    for (mut pos, vel) in query.iter() {
        for i in 0..3 {
            pos.0[i] += vel.0[i] * 0.016;
        }
    }
}

fn system_update(c: &mut Criterion) {
    GlobalTaskPool::get_or_init(TaskPool::new);
    let mut group = c.benchmark_group("system_update");
    group.sample_size(10);
    for n in SIZES {
        group.throughput(Throughput::Elements(n));
        let mut world = physics_world(n);
        world.push_update_stage(BenchStage);
        world.add_system(integrate, BenchStage);
        world.initialize_systems();
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| world.update().unwrap());
        });
    }
}

fn spawn(c: &mut Criterion) {
    let mut group = c.benchmark_group("spawn");
    for n in SIZES {
        group.throughput(Throughput::Elements(n));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                World::new,
                |world| {
                    for i in 0..n {
                        let f = i as f32;
                        world.spawn((Pos([f, f, f]), Vel([1.0, 0.5, 0.25])));
                    }
                    world
                },
                BatchSize::LargeInput,
            );
        });
    }
}

fn insert_remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_remove");
    group.sample_size(10);
    for n in SIZES {
        group.throughput(Throughput::Elements(n));
        let (world, entities) = world_with_a(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                for entity in &entities {
                    world.insert_component(*entity, B(1));
                }
                for entity in &entities {
                    black_box(world.remove_component::<B>(*entity));
                }
            });
        });
    }
}

fn despawn(c: &mut Criterion) {
    let mut group = c.benchmark_group("despawn");
    group.sample_size(10);
    for n in SIZES {
        group.throughput(Throughput::Elements(n));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || world_with_a(n),
                |(world, entities)| {
                    for entity in &entities {
                        world.destroy_entity(*entity);
                    }
                    world
                },
                BatchSize::LargeInput,
            );
        });
    }
}

criterion_group!(
    benches,
    query_new,
    iter_read,
    iter_write,
    iter_tuple,
    iter_optional,
    iter_fragmented,
    changed,
    get_all,
    system_update,
    spawn,
    insert_remove,
    despawn,
);
criterion_main!(benches);
