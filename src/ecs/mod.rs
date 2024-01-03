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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component)]
    struct TestComponent {
        value: u32,
    }

    #[derive(Resource)]
    struct TestResource {
        value: u32,
    }

    #[system(TestSystem)]
    fn test_system(test_resource: Res<TestResource>, mut query: Query<Write<TestComponent>>) {
        assert_eq!(test_resource.value, 42);
        for mut component in query.iter() {
            component.value += 1;
        }
    }

    #[test]
    fn test_ecs() -> anyhow::Result<()> {
        let mut world = World::new();
        let entity = world.create_entity();
        world.add_component(entity, TestComponent { value: 69 })?;
        assert!(world.has_component::<TestComponent>(entity));
        world.insert_resource(TestResource { value: 42 })?;
        world.add_system(TestSystem);
        world.update()?;
        assert_eq!(
            world
                .query::<Read<TestComponent>>()
                .iter()
                .next()
                .unwrap()
                .value,
            70
        );
        Ok(())
    }
}
