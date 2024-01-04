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
    query::{Query, QueryFilter},
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
    use rustc_hash::FxHashSet;

    use super::*;

    #[derive(Component, Debug)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Component, Debug)]
    struct Velocity {
        x: f32,
        y: f32,
    }

    #[derive(Bundle)]
    struct PhysicsBundle {
        position: Position,
        velocity: Velocity,
    }

    #[system(Physics)]
    fn physics(physics: Query<(&mut Position, &Velocity)>) {
        for (mut position, velocity) in physics.iter() {
            position.x += velocity.x;
            position.y += velocity.y;
        }
    }

    #[system(OptionPhysics)]
    fn option_physics(physics: Query<(&mut Position, Option<&Velocity>)>) {
        for (mut position, velocity) in physics.iter() {
            if let Some(velocity) = velocity {
                position.x += velocity.x;
                position.y += velocity.y;
            } else {
                position.x -= 1.0;
                position.y -= 1.0;
            }
        }
    }

    #[test]
    fn test_query_trait() -> anyhow::Result<()> {
        assert_eq!(
            <&Position>::reads(),
            Some(FxHashSet::from_iter(vec![Position::component_id()]))
        );
        assert_eq!(<&Position>::writes(), None);
        assert_eq!(
            <&mut Position>::writes(),
            Some(FxHashSet::from_iter(vec![Position::component_id()]))
        );
        assert_eq!(
            <&Velocity>::reads(),
            Some(FxHashSet::from_iter(vec![Velocity::component_id()]))
        );
        assert_eq!(<&Velocity>::writes(), None);
        assert_eq!(
            <&mut Velocity>::writes(),
            Some(FxHashSet::from_iter(vec![Velocity::component_id()]))
        );

        Ok(())
    }

    #[test]
    fn test_query_world() -> anyhow::Result<()> {
        let world = World::new();

        let entity = world.spawn(PhysicsBundle {
            position: Position { x: 0.0, y: 0.0 },
            velocity: Velocity { x: 1.0, y: 1.0 },
        })?;

        let position = world.query::<&Position>();
        let position = position.get(entity).unwrap();
        assert_eq!(position.x, 0.0);
        assert_eq!(position.y, 0.0);

        let velocity = world.query::<&Velocity>();
        let velocity = velocity.get(entity).unwrap();
        assert_eq!(velocity.x, 1.0);
        assert_eq!(velocity.y, 1.0);

        let physics = world.query::<(&Position, &Velocity)>();
        let (position, velocity) = physics.get(entity).unwrap();
        assert_eq!(position.x, 0.0);
        assert_eq!(position.y, 0.0);
        assert_eq!(velocity.x, 1.0);
        assert_eq!(velocity.y, 1.0);

        Ok(())
    }

    #[test]
    fn test_system() -> anyhow::Result<()> {
        let world = World::new();

        let entity = world.spawn(PhysicsBundle {
            position: Position { x: 0.0, y: 0.0 },
            velocity: Velocity { x: 1.0, y: 1.0 },
        })?;

        world.add_system(Physics);

        world.update()?;

        let position = world.query::<&Position>();
        let position = position.get(entity).unwrap();
        assert_eq!(position.x, 1.0);
        assert_eq!(position.y, 1.0);

        Ok(())
    }

    #[test]
    fn test_query_option() -> anyhow::Result<()> {
        let world = World::new();

        let entity = world.spawn(PhysicsBundle {
            position: Position { x: 0.0, y: 0.0 },
            velocity: Velocity { x: 1.0, y: 1.0 },
        })?;

        world.add_system(OptionPhysics);

        world.update()?;

        {
            let position = world.query::<&Position>();
            let position = position.get(entity).unwrap();
            assert_eq!(position.x, 1.0);
            assert_eq!(position.y, 1.0);
        }

        let entity2 = world.spawn(Position { x: 0.0, y: 0.0 })?;

        world.update()?;
        {
            let position = world.query::<&Position>();
            let position = position.get(entity2).unwrap();
            assert_eq!(position.x, -1.0);
            assert_eq!(position.y, -1.0);
        }

        Ok(())
    }
}
