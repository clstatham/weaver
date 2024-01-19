#![deny(unsafe_op_in_unsafe_fn)]

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

use parking_lot::Mutex;
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

pub type StaticId = u64;

#[derive(Clone, Copy)]
pub struct TypeInfo {
    pub(crate) id: StaticId,
    pub(crate) name: &'static str,
    pub(crate) drop_fn: unsafe fn(*mut u8),
    pub(crate) layout: std::alloc::Layout,
}

impl TypeInfo {
    pub fn of<T: Send + Sync + 'static>() -> Self {
        Self {
            id: static_id::<T>(),
            name: std::any::type_name::<T>(),
            drop_fn: |ptr| unsafe {
                std::ptr::drop_in_place::<T>(ptr.cast());
            },
            layout: std::alloc::Layout::new::<T>(),
        }
    }

    pub fn id(&self) -> StaticId {
        self.id
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn drop_fn(&self) -> unsafe fn(*mut u8) {
        self.drop_fn
    }

    pub fn layout(&self) -> std::alloc::Layout {
        self.layout
    }
}

lazy_static::lazy_static! {
    pub(crate) static ref TYPE_ID_MAP: Mutex<TypeIdMap<StaticId>> = Mutex::new(TypeIdMap::default());
}

#[inline]
pub fn static_id<T: 'static>() -> StaticId {
    let mut type_id_map = TYPE_ID_MAP.lock();

    let type_id = TypeId::of::<T>();

    if let Some(id) = type_id_map.get(&type_id) {
        *id
    } else {
        let id = type_id_map.len() as StaticId;
        type_id_map.insert(type_id, id);
        id
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

        println!("Adding entity 1 (A, B, C)");
        world.spawn((A::default(), B::default(), C::default()));
        println!("Adding entity 2 (A, B)");
        world.spawn((A::default(), B::default()));
        println!("Adding entity 3 (A, C)");
        world.spawn((A::default(), C::default()));
        println!("Adding entity 4 (A, B, C)");
        world.spawn((A::default(), B::default(), C::default()));

        dbg!(world.components.archetypes.len());

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

        let (a, b, c) = query.get(entity.id()).unwrap();

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

        let (a, b, c) = query.get(entity4.id()).unwrap();

        assert_eq!(a.a, 0);
        assert_eq!(b.b, 0);
        assert_eq!(c.c, 0);
    }

    #[test]
    fn test_query_get_filtered() {
        let mut world = World::new();

        let entity = world.spawn((A::default(), B::default(), C::default()));

        let query = world.query_filtered::<&B, With<A>>();

        let b = query.get(entity.id()).unwrap();

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

        let b = query.get(entity4.id()).unwrap();

        assert_eq!(b.b, 0);
    }
}
