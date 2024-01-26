#![deny(unsafe_op_in_unsafe_fn)]

pub mod bundle;
pub mod commands;
pub mod component;
pub mod component_impl;
pub mod entity;
pub mod query;
pub mod registry;
pub mod script;
pub mod storage;
pub mod system;
pub mod world;

pub mod prelude {
    pub use crate::{
        bundle::Bundle,
        commands::Commands,
        component::Component,
        entity::{Entity, EntityGraph},
        query::{Query, Queryable, With, Without},
        system::{System, SystemStage},
        world::World,
    };
    pub use rayon::prelude::*;
    pub use weaver_proc_macro::{system, Bundle, Component};
}

#[cfg(test)]
mod tests {
    #![allow(dead_code, unused)]
    use std::path::PathBuf;
    use std::sync::Arc;

    use parking_lot::RwLock;

    use crate as weaver_ecs;
    use crate::component::Data;
    use crate::prelude::*;
    use crate::query::DynamicQueryParams;
    use crate::script::interp::BuildOnWorld;
    use crate::script::Script;
    use crate::system::DynamicSystem;

    #[derive(Debug, Default, Component, Clone)]
    struct A {
        a: u32,
    }

    #[derive(Debug, Default, Component, Clone)]
    struct B {
        b: u32,
    }

    #[derive(Debug, Default, Component, Clone)]
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

        let query = world.query_filtered::<&B, With<A>>();

        let mut count = 0;

        for _ in query.iter() {
            count += 1;
        }

        assert_eq!(count, 3);
    }

    #[test]
    fn test_query_without() {
        let mut world = World::new();

        world.spawn((A::default(), B::default(), C::default()));
        world.spawn((A::default(), B::default()));
        world.spawn((A::default(), C::default()));
        world.spawn((A::default(), B::default(), C::default()));

        let query = world.query_filtered::<&B, Without<C>>();

        let mut count = 0;

        for _ in query.iter() {
            count += 1;
        }

        assert_eq!(count, 1);
    }

    #[test]
    fn test_query_get() {
        let mut world = World::new();

        let entity = world.spawn((A::default(), B::default(), C::default()));

        let query = world.query::<(&A, &B, &C)>();

        let (a, b, c) = query.get(entity).unwrap();

        assert_eq!(a.a, 0);
        assert_eq!(b.b, 0);
        assert_eq!(c.c, 0);
    }

    #[test]
    fn test_query_get_multiple_archetypes() {
        let mut world = World::new();

        let entity1 = world.spawn((A::default(), B::default(), C::default()));
        let entity2 = world.spawn((A::default(), B::default()));
        let entity3 = world.spawn((A::default(), C::default()));
        let entity4 = world.spawn((A::default(), B::default(), C::default()));

        let query = world.query::<(&A, &B, &C)>();

        let (a, b, c) = query.get(entity4).unwrap();

        assert_eq!(a.a, 0);
        assert_eq!(b.b, 0);
        assert_eq!(c.c, 0);
    }

    #[test]
    fn test_query_get_filtered() {
        let mut world = World::new();

        let entity = world.spawn((A::default(), B::default(), C::default()));

        let query = world.query_filtered::<&B, With<A>>();

        let b = query.get(entity).unwrap();

        assert_eq!(b.b, 0);
    }

    #[test]
    fn test_query_get_filtered_multiple_archetypes() {
        let mut world = World::new();

        let entity1 = world.spawn((A::default(), B::default(), C::default()));
        let entity2 = world.spawn((A::default(), B::default()));
        let entity3 = world.spawn((A::default(), C::default()));
        let entity4 = world.spawn((A::default(), B::default(), C::default()));

        let query = world.query_filtered::<&B, With<A>>();

        let b = query.get(entity4).unwrap();

        assert_eq!(b.b, 0);
    }

    #[test]
    fn test_query_dynamic() {
        let mut world = World::new();

        world.spawn((A::default(), B::default(), C::default()));
        world.spawn((A::default(), B::default()));
        world.spawn((A::default(), C::default()));
        world.spawn((A::default(), B::default(), C::default()));

        let query = world
            .query_dynamic()
            .read::<A>()
            .read::<B>()
            .read::<C>()
            .build();

        let mut count = 0;

        for entry in query.iter() {
            count += 1;
        }

        assert_eq!(count, 2);
    }

    #[test]
    fn test_query_dynamic_ids() {
        let mut world = World::new();

        world.spawn((A::default(), B::default(), C::default()));
        world.spawn((A::default(), B::default()));
        world.spawn((A::default(), C::default()));
        world.spawn((A::default(), B::default(), C::default()));

        let query = world
            .query_dynamic()
            .read_id(world.dynamic_id::<A>())
            .read_id(world.dynamic_id::<B>())
            .read_id(world.dynamic_id::<C>())
            .build();

        let mut count = 0;

        for entry in query.iter() {
            count += 1;
        }

        assert_eq!(count, 2);
    }
}
