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

    #[derive(Component, Debug)]
    struct Acceleration {
        x: f32,
        y: f32,
    }

    #[derive(Resource, Debug)]
    struct Time {
        delta: f32,
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

        let entity = world.spawn((Position { x: 0.0, y: 0.0 }, Velocity { x: 1.0, y: 1.0 }))?;

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

    #[system(Physics)]
    fn physics(physics: Query<(&mut Position, &mut Velocity, &Acceleration)>, time: Res<Time>) {
        for (mut position, mut velocity, acceleration) in physics.iter() {
            position.x += velocity.x * time.delta;
            position.y += velocity.y * time.delta;

            velocity.x += acceleration.x * time.delta;
            velocity.y += acceleration.y * time.delta;
        }
    }

    #[test]
    fn test_system() -> anyhow::Result<()> {
        let mut world = World::new();
        world.insert_resource(Time { delta: 0.5 })?;

        let entity = world.spawn((
            Position { x: 0.0, y: 0.0 },
            Velocity { x: 1.0, y: 1.0 },
            Acceleration { x: 1.0, y: 1.0 },
        ))?;

        world.add_system(Physics);

        world.startup()?;
        world.update()?;

        {
            let position = Query::<&Position>::new(&world);
            let position = position.get(entity).unwrap();
            assert_eq!(position.x, 0.5);
            assert_eq!(position.y, 0.5);
        }

        {
            let velocity = Query::<&Velocity>::new(&world);
            let velocity = velocity.get(entity).unwrap();
            assert_eq!(velocity.x, 1.5);
            assert_eq!(velocity.y, 1.5);
        }

        Ok(())
    }

    #[system(PositionUpdate)]
    fn position_update(query: Query<(&mut Position, &Velocity)>, time: Res<Time>) {
        for (mut position, velocity) in query.iter() {
            position.x += velocity.x * time.delta;
            position.y += velocity.y * time.delta;
        }
    }

    #[system(VelocityUpdate)]
    fn velocity_update(query: Query<(&mut Velocity, &Acceleration)>, time: Res<Time>) {
        for (mut velocity, acceleration) in query.iter() {
            velocity.x += acceleration.x * time.delta;
            velocity.y += acceleration.y * time.delta;
        }
    }

    #[test]
    fn test_system_order() -> anyhow::Result<()> {
        let mut world = World::new();
        world.insert_resource(Time { delta: 0.5 })?;

        let entity = world.spawn((
            Position { x: 0.0, y: 0.0 },
            Velocity { x: 1.0, y: 1.0 },
            Acceleration { x: 1.0, y: 1.0 },
        ))?;

        world.add_system(VelocityUpdate);
        world.add_system(PositionUpdate);

        world.startup()?;
        world.update()?;

        {
            let position = Query::<&Position>::new(&world);
            let position = position.get(entity).unwrap();
            assert_eq!(position.x, 0.75);
            assert_eq!(position.y, 0.75);
        }

        {
            let velocity = Query::<&Velocity>::new(&world);
            let velocity = velocity.get(entity).unwrap();
            assert_eq!(velocity.x, 1.5);
            assert_eq!(velocity.y, 1.5);
        }

        Ok(())
    }

    #[test]
    fn test_system_dependency() -> anyhow::Result<()> {
        let mut world = World::new();
        world.insert_resource(Time { delta: 0.5 })?;

        let entity = world.spawn((
            Position { x: 0.0, y: 0.0 },
            Velocity { x: 1.0, y: 1.0 },
            Acceleration { x: 1.0, y: 1.0 },
        ))?;

        // do them out of order to make sure the dependency is respected
        let sys = world.add_system(PositionUpdate);
        world.add_system_after(VelocityUpdate, sys);

        world.startup()?;
        world.update()?;

        {
            let position = Query::<&Position>::new(&world);
            let position = position.get(entity).unwrap();
            assert_eq!(position.x, 0.5);
            assert_eq!(position.y, 0.5);
        }

        {
            let velocity = Query::<&Velocity>::new(&world);
            let velocity = velocity.get(entity).unwrap();
            assert_eq!(velocity.x, 1.5);
            assert_eq!(velocity.y, 1.5);
        }

        Ok(())
    }

    #[test]
    fn test_system_dependency_cycle() -> anyhow::Result<()> {
        let mut world = World::new();
        world.insert_resource(Time { delta: 0.5 })?;

        let _entity = world.spawn((
            Position { x: 0.0, y: 0.0 },
            Velocity { x: 1.0, y: 1.0 },
            Acceleration { x: 1.0, y: 1.0 },
        ))?;

        let sys = world.add_system(PositionUpdate);
        let sys2 = world.add_system(VelocityUpdate);
        world.add_system_dependency(sys, sys2);
        world.add_system_dependency(sys2, sys);

        assert!(world.update().is_err());

        Ok(())
    }

    #[system(OptionPhysics)]
    fn option_physics(
        query: Query<(&mut Position, &mut Velocity, Option<&Acceleration>)>,
        time: Res<Time>,
    ) {
        for (mut position, mut velocity, acceleration) in query.iter() {
            position.x += velocity.x * time.delta;
            position.y += velocity.y * time.delta;

            if let Some(acceleration) = acceleration {
                velocity.x += acceleration.x * time.delta;
                velocity.y += acceleration.y * time.delta;
            }
        }
    }

    #[test]
    fn test_option_query() -> anyhow::Result<()> {
        let mut world = World::new();
        world.insert_resource(Time { delta: 0.5 })?;

        let no_accel = world.spawn((Position { x: 0.0, y: 0.0 }, Velocity { x: 1.0, y: 1.0 }))?;
        let with_accel = world.spawn((
            Position { x: 0.0, y: 0.0 },
            Velocity { x: 1.0, y: 1.0 },
            Acceleration { x: 1.0, y: 1.0 },
        ))?;

        world.add_system(OptionPhysics);

        world.startup()?;
        world.update()?;

        {
            let position = Query::<&Position>::new(&world);
            let position = position.get(no_accel).unwrap();
            assert_eq!(position.x, 0.5);
            assert_eq!(position.y, 0.5);
        }

        {
            let velocity = Query::<&Velocity>::new(&world);
            let velocity = velocity.get(no_accel).unwrap();
            assert_eq!(velocity.x, 1.0);
            assert_eq!(velocity.y, 1.0);
        }

        {
            let position = Query::<&Position>::new(&world);
            let position = position.get(with_accel).unwrap();
            assert_eq!(position.x, 0.5);
            assert_eq!(position.y, 0.5);
        }

        {
            let velocity = Query::<&Velocity>::new(&world);
            let velocity = velocity.get(with_accel).unwrap();
            assert_eq!(velocity.x, 1.5);
            assert_eq!(velocity.y, 1.5);
        }

        Ok(())
    }

    #[system(WithSimple)]
    fn with_simple(query: Query<&Position, With<Acceleration>>) {
        dbg!(&query.entries);
        for position in query.iter() {
            assert_eq!(position.x, 1.0);
            assert_eq!(position.y, 1.0);
        }
    }

    #[test]
    fn test_with_simple_query() -> anyhow::Result<()> {
        let world = World::new();

        let no_accel = world.spawn((Position { x: 0.0, y: 0.0 }, Velocity { x: 1.0, y: 1.0 }))?;
        let with_accel = world.spawn((
            Position { x: 1.0, y: 1.0 },
            Velocity { x: 1.0, y: 1.0 },
            Acceleration { x: 1.0, y: 1.0 },
        ))?;

        world.add_system(WithSimple);

        world.startup()?;
        world.update()?;

        {
            let position = Query::<&Position>::new(&world);
            let position = position.get(no_accel).unwrap();
            assert_eq!(position.x, 0.0);
            assert_eq!(position.y, 0.0);
        }

        {
            let position = Query::<&Position>::new(&world);
            let position = position.get(with_accel).unwrap();
            assert_eq!(position.x, 1.0);
            assert_eq!(position.y, 1.0);
        }

        Ok(())
    }

    #[system(WithPhysics)]
    fn with_physics(query: Query<(&mut Position, &Velocity), With<Acceleration>>, time: Res<Time>) {
        for (mut position, velocity) in query.iter() {
            position.x += velocity.x * time.delta;
            position.y += velocity.y * time.delta;
        }
    }

    #[test]
    fn test_with_query() -> anyhow::Result<()> {
        let mut world = World::new();
        world.insert_resource(Time { delta: 0.5 })?;

        let no_accel = world.spawn((Position { x: 0.0, y: 0.0 }, Velocity { x: 1.0, y: 1.0 }))?;
        let with_accel = world.spawn((
            Position { x: 0.0, y: 0.0 },
            Velocity { x: 1.0, y: 1.0 },
            Acceleration { x: 1.0, y: 1.0 },
        ))?;

        world.add_system(WithPhysics);

        world.startup()?;
        world.update()?;

        {
            let position_query = Query::<&Position>::new(&world);

            let position = position_query.get(no_accel).unwrap();
            assert_eq!(position.x, 0.0);
            assert_eq!(position.y, 0.0);

            let position = position_query.get(with_accel).unwrap();
            assert_eq!(position.x, 0.5);
            assert_eq!(position.y, 0.5);
        }

        Ok(())
    }

    #[system(WithoutPhysics)]
    fn without_physics(
        query: Query<(&mut Position, &Velocity), Without<Acceleration>>,
        time: Res<Time>,
    ) {
        for (mut position, velocity) in query.iter() {
            position.x += velocity.x * time.delta;
            position.y += velocity.y * time.delta;
        }
    }

    #[test]
    fn test_without_query() -> anyhow::Result<()> {
        let mut world = World::new();
        world.insert_resource(Time { delta: 0.5 })?;

        let no_accel = world.spawn((Position { x: 0.0, y: 0.0 }, Velocity { x: 1.0, y: 1.0 }))?;
        let with_accel = world.spawn((
            Position { x: 0.0, y: 0.0 },
            Velocity { x: 1.0, y: 1.0 },
            Acceleration { x: 1.0, y: 1.0 },
        ))?;

        world.add_system(WithoutPhysics);

        world.startup()?;
        world.update()?;

        {
            let position = Query::<&Position>::new(&world);
            let position = position.get(no_accel).unwrap();
            assert_eq!(position.x, 0.5);
            assert_eq!(position.y, 0.5);
        }

        {
            let velocity = Query::<&Velocity>::new(&world);
            let velocity = velocity.get(no_accel).unwrap();
            assert_eq!(velocity.x, 1.0);
            assert_eq!(velocity.y, 1.0);
        }

        {
            let position = Query::<&Position>::new(&world);
            let position = position.get(with_accel).unwrap();
            assert_eq!(position.x, 0.0);
            assert_eq!(position.y, 0.0);
        }

        {
            let velocity = Query::<&Velocity>::new(&world);
            let velocity = velocity.get(with_accel).unwrap();
            assert_eq!(velocity.x, 1.0);
            assert_eq!(velocity.y, 1.0);
        }

        Ok(())
    }
}
