pub mod bundle;
pub mod component;
pub mod entity;
pub mod graph;
pub mod query;
pub mod resource;
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

use thiserror::Error;
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

#[derive(Debug, Error)]
#[error("An ECS error occurred")]
pub enum EcsError {
    #[error("A component with the same ID already exists")]
    ComponentAlreadyExists,
    #[error("Component does not exist for entity")]
    ComponentDoesNotExist,
    #[error("Resource already exists in world")]
    ResourceAlreadyExists,
    #[error("Resource does not exist in world")]
    ResourceDoesNotExist,
    #[error("Entity does not exist in world")]
    EntityDoesNotExist,
    #[error("System already exists in world")]
    SystemAlreadyExists,
    #[error("System does not exist in world")]
    SystemDoesNotExist,
    #[error("System dependency does not exist in world")]
    SystemDependencyDoesNotExist,
    #[error("System dependency cycle detected")]
    SystemDependencyCycleDetected,
    #[error("System panicked")]
    SystemPanicked,
}
