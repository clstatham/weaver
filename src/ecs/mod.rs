pub mod archetype;
pub mod bundle;
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
    component::Component,
    entity::Entity,
    query::{Query, Queryable, With, Without},
    resource::{Res, ResMut, Resource},
    system::System,
    world::World,
};

use std::sync::Arc;

pub use weaver_proc_macro::{system, Bundle, Component, Resource, StaticId};

/// A unique identifier for something that is known at compile time.
/// This is used to identify components, resources, and systems.
///
/// # Safety
///
/// This trait is unsafe because it is up to the implementor to ensure that
/// the ID is unique.
pub unsafe trait StaticId {
    fn static_id() -> usize
    where
        Self: Sized;
}

unsafe impl<T: StaticId> StaticId for Option<T> {
    fn static_id() -> usize
    where
        Self: Sized,
    {
        T::static_id()
    }
}

unsafe impl<T: StaticId> StaticId for Vec<T> {
    fn static_id() -> usize
    where
        Self: Sized,
    {
        T::static_id()
    }
}

unsafe impl<T: StaticId> StaticId for Box<T> {
    fn static_id() -> usize
    where
        Self: Sized,
    {
        T::static_id()
    }
}

unsafe impl<T: StaticId> StaticId for Arc<T> {
    fn static_id() -> usize
    where
        Self: Sized,
    {
        T::static_id()
    }
}

unsafe impl<T: StaticId> StaticId for &T {
    fn static_id() -> usize
    where
        Self: Sized,
    {
        T::static_id()
    }
}

unsafe impl<T: StaticId> StaticId for &mut T {
    fn static_id() -> usize
    where
        Self: Sized,
    {
        T::static_id()
    }
}

unsafe impl<T: StaticId> StaticId for parking_lot::RwLock<T> {
    fn static_id() -> usize
    where
        Self: Sized,
    {
        T::static_id()
    }
}
