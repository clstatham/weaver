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
    query::{Query, Queryable},
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

    use super::{
        query::{With, Without},
        *,
    };

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

    #[test]
    fn test_query_trait() -> anyhow::Result<()> {
        assert_eq!(
            <&Position as Queryable<()>>::reads(),
            Some(FxHashSet::from_iter(vec![Position::component_id()]))
        );
        assert_eq!(<&Position as Queryable<()>>::writes(), None);
        assert_eq!(
            <&mut Position as Queryable<()>>::writes(),
            Some(FxHashSet::from_iter(vec![Position::component_id()]))
        );
        assert_eq!(
            <&Velocity as Queryable<()>>::reads(),
            Some(FxHashSet::from_iter(vec![Velocity::component_id()]))
        );
        assert_eq!(<&Velocity as Queryable<()>>::writes(), None);
        assert_eq!(
            <&mut Velocity as Queryable<()>>::writes(),
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

        let position = Query::<&Position>::new(&world);
        let position = position.get(entity).unwrap();
        assert_eq!(position.x, 0.0);
        assert_eq!(position.y, 0.0);

        let velocity = Query::<&Velocity>::new(&world);
        let velocity = velocity.get(entity).unwrap();
        assert_eq!(velocity.x, 1.0);
        assert_eq!(velocity.y, 1.0);

        let physics = Query::<(&Position, &Velocity)>::new(&world);
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

        let position = Query::<&Position>::new(&world);
        let position = position.get(entity).unwrap();
        assert_eq!(position.x, 1.0);
        assert_eq!(position.y, 1.0);

        Ok(())
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
    fn test_query_option() -> anyhow::Result<()> {
        let world = World::new();

        let entity = world.spawn(PhysicsBundle {
            position: Position { x: 0.0, y: 0.0 },
            velocity: Velocity { x: 1.0, y: 1.0 },
        })?;

        world.add_system(OptionPhysics);

        world.update()?;

        {
            let position = Query::<&Position>::new(&world);
            let position = position.get(entity).unwrap();
            assert_eq!(position.x, 1.0);
            assert_eq!(position.y, 1.0);
        }

        let entity2 = world.spawn(Position { x: 0.0, y: 0.0 })?;

        world.update()?;
        {
            let position = Query::<&Position>::new(&world);
            let position = position.get(entity2).unwrap();
            assert_eq!(position.x, -1.0);
            assert_eq!(position.y, -1.0);
        }

        Ok(())
    }

    #[system(WithPhysics)]
    fn with_physics(physics: Query<&Position, With<Velocity>>) {
        for position in physics.iter() {
            assert_eq!(position.x, 1.0);
            assert_eq!(position.y, 1.0);
        }
    }

    #[test]
    fn test_query_with() -> anyhow::Result<()> {
        let world = World::new();

        let _entity = world.spawn(PhysicsBundle {
            position: Position { x: 1.0, y: 1.0 },
            velocity: Velocity { x: 1.0, y: 1.0 },
        })?;
        let _entity2 = world.spawn(Position { x: 0.0, y: 0.0 })?;

        world.add_system(WithPhysics);

        world.update()?;

        Ok(())
    }

    #[system(WithoutPhysics)]
    fn without_physics(physics: Query<&Position, Without<Velocity>>) {
        for position in physics.iter() {
            assert_eq!(position.x, 1.0);
            assert_eq!(position.y, 1.0);
        }
    }

    #[test]
    fn test_query_without() -> anyhow::Result<()> {
        let world = World::new();

        let _entity = world.spawn(Position { x: 1.0, y: 1.0 })?;
        let _entity2 = world.spawn(PhysicsBundle {
            position: Position { x: 0.0, y: 0.0 },
            velocity: Velocity { x: 1.0, y: 1.0 },
        })?;

        world.add_system(WithoutPhysics);

        world.update()?;

        Ok(())
    }
}
