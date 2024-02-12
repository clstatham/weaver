use crate::{self as fabricate, prelude::*};

/// Tag component for entities that are considered to be components of another entity.
#[derive(Debug, Clone, Component)]
pub struct IsComponent;

/// Relationship for components that are considered to be part of another component or entity.
///
/// Used to implement component types that are created at runtime.
#[derive(Debug, Clone, Component)]
pub struct Has {
    pub name: String,
}

impl Relationship for Has {}

impl Has {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}
