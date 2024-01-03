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
    query::{Query, Queryable, Read, Write},
    resource::{Res, ResMut, Resource},
    system::System,
    world::World,
};

use thiserror::Error;
pub use weaver_proc_macro::{system, Bundle, Component, Resource};

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
}
