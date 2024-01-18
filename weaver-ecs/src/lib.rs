pub mod archetype;
pub mod bundle;
pub mod commands;
pub mod component;
pub mod entity;
pub mod graph;
pub mod query;
pub mod resource;
pub mod storage;
pub mod system;
pub mod world;

pub use {
    bundle::Bundle,
    commands::Commands,
    component::Component,
    entity::Entity,
    query::{Query, Queryable, With, Without},
    resource::{Res, ResMut, Resource},
    system::{System, SystemStage},
    world::World,
};

use std::{any::TypeId, collections::HashMap, hash::BuildHasherDefault};

use rustc_hash::FxHasher;
pub use weaver_proc_macro::{system, Bundle, Component, Resource};

#[derive(Default)]
pub struct TypeIdHasher(u64);

impl std::hash::Hasher for TypeIdHasher {
    fn write_u64(&mut self, i: u64) {
        debug_assert_eq!(self.0, 0);
        self.0 = i;
    }

    fn write_u128(&mut self, i: u128) {
        debug_assert_eq!(self.0, 0);
        self.0 = i as u64;
    }

    fn write(&mut self, bytes: &[u8]) {
        debug_assert_eq!(self.0, 0);

        let mut hasher = FxHasher::default();
        hasher.write(bytes);
        self.0 = hasher.finish();
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

pub(crate) type TypeIdMap<T> = HashMap<TypeId, T, BuildHasherDefault<TypeIdHasher>>;

#[cfg(test)]
mod tests {
    #![allow(dead_code, unused)]
    use super::*;
    use crate as weaver_ecs;

    #[derive(Debug, Default, Component)]
    struct A {
        a: u32,
    }

    #[derive(Debug, Default, Component)]
    struct B {
        b: u32,
    }

    #[derive(Debug, Default, Component)]
    struct C {
        c: u32,
    }

    #[test]
    fn test_query() {
        let mut world = World::new();

        world.spawn((A::default(), B::default(), C::default()));
        world.spawn((A::default(), B::default()));
        world.spawn((A::default(), C::default()));
        world.spawn((A::default(), B::default(), C::default()));

        let query = world.query::<(&A, &B, &C)>();

        let mut count = 0;

        for (a, b, c) in query.iter() {
            count += 1;
        }

        assert_eq!(count, 2);
    }

    #[test]
    fn test_query_with() {
        let mut world = World::new();

        world.spawn((A::default(), B::default(), C::default()));
        world.spawn((A::default(), B::default()));
        world.spawn((A::default(), C::default()));
        world.spawn((A::default(), B::default(), C::default()));

        let query = world.query_filtered::<(), With<A>>();

        let mut count = 0;

        for _ in query.iter() {
            count += 1;
        }

        assert_eq!(count, 4);
    }

    #[test]
    fn test_query_without() {
        let mut world = World::new();

        world.spawn((A::default(), B::default(), C::default()));
        world.spawn((A::default(), B::default()));
        world.spawn((A::default(), C::default()));
        world.spawn((A::default(), B::default(), C::default()));

        dbg!(&world.components.archetypes);

        let query = world.query_filtered::<(), Without<C>>();

        let mut count = 0;

        for _ in query.iter() {
            count += 1;
        }

        assert_eq!(count, 1);
    }
}
