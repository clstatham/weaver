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

use std::sync::Arc;

pub use weaver_proc_macro::{system, Bundle, Component, Resource, StaticId};

/// A unique identifier for something that is known at compile time.
/// This is used to identify components and resources.
///
/// # Safety
///
/// This trait is unsafe because it is up to the implementor to ensure that
/// the ID is unique.
pub unsafe trait StaticId {
    fn static_id() -> u128
    where
        Self: Sized;

    fn static_name() -> &'static str
    where
        Self: Sized,
    {
        std::any::type_name::<Self>()
    }
}

unsafe impl<T: StaticId> StaticId for Option<T> {
    fn static_id() -> u128
    where
        Self: Sized,
    {
        T::static_id()
    }
}

unsafe impl<T: StaticId> StaticId for Vec<T> {
    fn static_id() -> u128
    where
        Self: Sized,
    {
        T::static_id()
    }
}

unsafe impl<T: StaticId> StaticId for Box<T> {
    fn static_id() -> u128
    where
        Self: Sized,
    {
        T::static_id()
    }
}

unsafe impl<T: StaticId> StaticId for Arc<T> {
    fn static_id() -> u128
    where
        Self: Sized,
    {
        T::static_id()
    }
}

unsafe impl<T: StaticId> StaticId for &T {
    fn static_id() -> u128
    where
        Self: Sized,
    {
        T::static_id()
    }
}

unsafe impl<T: StaticId> StaticId for &mut T {
    fn static_id() -> u128
    where
        Self: Sized,
    {
        T::static_id()
    }
}

unsafe impl<T: StaticId> StaticId for parking_lot::RwLock<T> {
    fn static_id() -> u128
    where
        Self: Sized,
    {
        T::static_id()
    }
}

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
