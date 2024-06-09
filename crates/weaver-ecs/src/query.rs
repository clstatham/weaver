use std::{any::TypeId, collections::HashSet};

use super::{
    component::Component,
    entity::Entity,
    storage::{Mut, Ref},
    world::World,
};

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Query {
    read: Vec<TypeId>,
    write: Vec<TypeId>,
    without: Vec<TypeId>,
    with: Vec<TypeId>,
}

impl Query {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read<T: Component>(mut self) -> Self {
        self.read.push(TypeId::of::<T>());
        self
    }

    pub fn write<T: Component>(mut self) -> Self {
        self.write.push(TypeId::of::<T>());
        self
    }

    pub fn without<T: Component>(mut self) -> Self {
        self.without.push(TypeId::of::<T>());
        self
    }

    pub fn with<T: Component>(mut self) -> Self {
        self.with.push(TypeId::of::<T>());
        self
    }

    pub fn get<'a>(&self, world: &'a World) -> QueryResults<'a> {
        let mut entities = Vec::new();

        for archetype in world.storage().read().archetype_iter() {
            let mut matches = true;
            if self
                .read
                .iter()
                .any(|t| !archetype.has_component_by_type_id(*t))
            {
                matches = false;
            }
            if self
                .write
                .iter()
                .any(|t| !archetype.has_component_by_type_id(*t))
            {
                matches = false;
            }
            if self
                .without
                .iter()
                .any(|t| archetype.has_component_by_type_id(*t))
            {
                matches = false;
            }
            if self
                .with
                .iter()
                .any(|t| !archetype.has_component_by_type_id(*t))
            {
                matches = false;
            }
            if matches {
                entities.extend(archetype.entity_iter());
            }
        }

        QueryResults {
            world,
            entities: entities.into_iter().collect(),
        }
    }
}

pub struct QueryResults<'a> {
    world: &'a World,
    entities: Vec<Entity>,
}

impl<'a> QueryResults<'a> {
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entities.iter().copied()
    }

    pub fn get<T: Component>(&self, entity: Entity) -> Option<Ref<T>> {
        if self.entities.contains(&entity) {
            self.world.get_component::<T>(entity)
        } else {
            None
        }
    }

    pub fn get_mut<T: Component>(&self, entity: Entity) -> Option<Mut<T>> {
        if self.entities.contains(&entity) {
            self.world.get_component_mut::<T>(entity)
        } else {
            None
        }
    }

    pub fn has<T: Component>(&self, entity: Entity) -> bool {
        self.entities.contains(&entity) && self.world.has_component::<T>(entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, PartialEq)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Debug, Default, PartialEq)]
    struct Velocity {
        x: f32,
        y: f32,
    }

    #[derive(Debug, Default, PartialEq)]
    struct Acceleration {
        x: f32,
        y: f32,
    }

    #[test]
    fn query() {
        let world = World::new();
        let entity1 = world.create_entity();
        let entity2 = world.create_entity();
        let entity3 = world.create_entity();

        world.insert_component(entity1, Position { x: 0.0, y: 0.0 });
        world.insert_component(entity1, Velocity { x: 1.0, y: 1.0 });

        world.insert_component(entity2, Position { x: 0.0, y: 0.0 });
        world.insert_component(entity2, Acceleration { x: 1.0, y: 1.0 });

        world.insert_component(entity3, Position { x: 0.0, y: 0.0 });
        world.insert_component(entity3, Velocity { x: 1.0, y: 1.0 });
        world.insert_component(entity3, Acceleration { x: 1.0, y: 1.0 });

        let query = Query::new().read::<Position>().write::<Velocity>();
        let results = query.get(&world);

        let entities = results.iter().collect::<Vec<_>>();
        assert!(entities.contains(&entity1));
        assert!(!entities.contains(&entity2));
        assert!(entities.contains(&entity3));

        assert!(results.has::<Position>(entity1));
        assert!(results.has::<Velocity>(entity1));
        assert!(!results.has::<Acceleration>(entity1));
        assert!(!results.has::<Position>(entity2));
        assert!(!results.has::<Velocity>(entity2));
        assert!(!results.has::<Acceleration>(entity2));
        assert!(results.has::<Position>(entity3));
        assert!(results.has::<Velocity>(entity3));
        assert!(results.has::<Acceleration>(entity3));

        assert_eq!(
            results.get::<Position>(entity1).as_deref(),
            Some(&*world.get_component::<Position>(entity1).unwrap())
        );
        assert_eq!(
            results.get::<Velocity>(entity1).as_deref(),
            Some(&*world.get_component::<Velocity>(entity1).unwrap())
        );
        assert_eq!(results.get::<Acceleration>(entity1).as_deref(), None);
        assert_eq!(results.get::<Position>(entity2).as_deref(), None);
        assert_eq!(results.get::<Velocity>(entity2).as_deref(), None);
        assert_eq!(results.get::<Acceleration>(entity2).as_deref(), None);
        assert_eq!(
            results.get::<Position>(entity3).as_deref(),
            Some(&*world.get_component::<Position>(entity3).unwrap())
        );
        assert_eq!(
            results.get::<Velocity>(entity3).as_deref(),
            Some(&*world.get_component::<Velocity>(entity3).unwrap())
        );
    }

    #[test]
    fn query_multiple_reads() {
        let world = World::new();
        let entity1 = world.create_entity();
        let entity2 = world.create_entity();
        let entity3 = world.create_entity();

        world.insert_component(entity1, Position { x: 0.0, y: 0.0 });
        world.insert_component(entity1, Velocity { x: 1.0, y: 1.0 });

        world.insert_component(entity2, Position { x: 0.0, y: 0.0 });
        world.insert_component(entity2, Acceleration { x: 1.0, y: 1.0 });

        world.insert_component(entity3, Position { x: 0.0, y: 0.0 });
        world.insert_component(entity3, Velocity { x: 1.0, y: 1.0 });
        world.insert_component(entity3, Acceleration { x: 1.0, y: 1.0 });

        let query = Query::new().read::<Position>().read::<Velocity>();
        let results = query.get(&world);
        let entities = results.iter().collect::<Vec<_>>();

        assert!(entities.contains(&entity1));
        assert!(!entities.contains(&entity2));
        assert!(entities.contains(&entity3));

        assert!(results.has::<Position>(entity1));
        assert!(results.has::<Velocity>(entity1));
        assert!(!results.has::<Acceleration>(entity1));

        assert!(!results.has::<Position>(entity2));
        assert!(!results.has::<Velocity>(entity2));
        assert!(!results.has::<Acceleration>(entity2));

        assert!(results.has::<Position>(entity3));
        assert!(results.has::<Velocity>(entity3));
        // assert!(!results.has::<Acceleration>(entity3));
    }
}
