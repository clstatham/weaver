use std::{any::TypeId, marker::PhantomData, sync::Arc};

use crate::prelude::Archetype;

use super::{
    component::Component,
    entity::Entity,
    storage::{Mut, Ref},
    world::World,
};

pub enum QueryAccess {
    ReadOnly,
    ReadWrite,
}

pub trait QueryFilterParam {
    type Item: Component;
    type Fetch<'a>;
    fn type_id() -> TypeId;
    fn access() -> QueryAccess;
    fn fetch<'a>(world: &World, entity: Entity) -> Option<Self::Fetch<'a>>;
}

impl<T: Component> QueryFilterParam for &T {
    type Item = T;
    type Fetch<'a> = Ref<T>;

    fn type_id() -> TypeId {
        TypeId::of::<T>()
    }

    fn access() -> QueryAccess {
        QueryAccess::ReadOnly
    }

    fn fetch<'a>(world: &World, entity: Entity) -> Option<Self::Fetch<'a>> {
        world.get_component::<T>(entity)
    }
}

impl<T: Component> QueryFilterParam for &mut T {
    type Item = T;
    type Fetch<'a> = Mut<T>;

    fn type_id() -> TypeId {
        TypeId::of::<T>()
    }

    fn access() -> QueryAccess {
        QueryAccess::ReadWrite
    }

    fn fetch<'a>(world: &World, entity: Entity) -> Option<Self::Fetch<'a>> {
        world.get_component_mut::<T>(entity)
    }
}

pub trait QueryFilter {
    type Fetch<'a>;
    fn access() -> &'static [(TypeId, QueryAccess)];
    fn fetch<'a>(world: &World, entity: Entity) -> Option<Self::Fetch<'a>>;
    fn test(world: &World, entity: Entity) -> bool;
    fn test_archetype(archetype: &Archetype) -> bool;
}

impl<T> QueryFilter for T
where
    T: QueryFilterParam,
{
    type Fetch<'a> = T::Fetch<'a>;

    fn access() -> &'static [(TypeId, QueryAccess)] {
        static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryAccess)>> = std::sync::OnceLock::new();
        ACCESS.get_or_init(|| vec![(T::type_id(), T::access())])
    }

    fn fetch<'a>(world: &World, entity: Entity) -> Option<Self::Fetch<'a>> {
        <T as QueryFilterParam>::fetch(world, entity)
    }

    fn test(world: &World, entity: Entity) -> bool {
        world.has_component::<T::Item>(entity)
    }

    fn test_archetype(archetype: &Archetype) -> bool {
        archetype.contains_component_by_type_id(TypeId::of::<T::Item>())
    }
}

macro_rules! impl_query_filter {
    ($($param:ident),*) => {
        impl<$($param: QueryFilterParam),*> QueryFilter for ($($param,)*) {
            type Fetch<'a> = ($($param::Fetch<'a>,)*);

            fn access() -> &'static [(TypeId, QueryAccess)] {
                static ACCESS: std::sync::OnceLock<Vec<(TypeId, QueryAccess)>> = std::sync::OnceLock::new();
                ACCESS.get_or_init(|| vec![$(($param::type_id(), $param::access()),)*])
            }

            #[allow(non_snake_case)]
            fn fetch<'a>(world: &World, entity: Entity) -> Option<Self::Fetch<'a>> {
                let ($($param,)*) = ($($param::fetch(world, entity)?,)*);
                Some(($($param,)*))

            }

            fn test(world: &World, entity: Entity) -> bool {
                $(
                    $param::test(world, entity) &&
                )*
                true
            }

            fn test_archetype(archetype: &Archetype) -> bool {
                $(
                    $param::test_archetype(archetype) &&
                )*
                true
            }
        }
    };
}

impl_query_filter!(A);
impl_query_filter!(A, B);
impl_query_filter!(A, B, C);
impl_query_filter!(A, B, C, D);
impl_query_filter!(A, B, C, D, E);
impl_query_filter!(A, B, C, D, E, F);
impl_query_filter!(A, B, C, D, E, F, G);
impl_query_filter!(A, B, C, D, E, F, G, H);

pub struct Query<Q: QueryFilter + ?Sized> {
    world: Arc<World>,
    entities: Box<[Entity]>,
    _phantom: PhantomData<Q>,
}

impl<Q: QueryFilter + ?Sized> Query<Q> {
    pub fn new(world: Arc<World>) -> Self {
        let mut entities = Vec::new();
        let storage = world.storage().read();

        for archetype in storage.archetype_iter() {
            if Q::test_archetype(archetype) {
                entities.extend(archetype.entity_iter());
            }
        }

        drop(storage);

        Self {
            world,
            entities: entities.into_boxed_slice(),
            _phantom: PhantomData,
        }
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entities.iter().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Entity, Q::Fetch<'_>)> + '_ {
        self.entities
            .iter()
            .filter_map(move |entity| Some((*entity, Q::fetch(&self.world, *entity)?)))
    }

    pub fn get(&self, entity: Entity) -> Option<Q::Fetch<'_>> {
        Q::fetch(&self.world, entity)
    }
}

#[cfg(test)]
mod tests {
    use crate as weaver_ecs;
    use weaver_ecs_macros::Component;

    use super::*;

    #[derive(Debug, Default, PartialEq, Component)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Debug, Default, PartialEq, Component)]
    struct Velocity {
        x: f32,
        y: f32,
    }

    #[derive(Debug, Default, PartialEq, Component)]
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

        let results = Query::<(&Position, &Velocity)>::new(world.clone());

        let entities = results.entity_iter().collect::<Vec<_>>();
        assert!(entities.contains(&entity1));
        assert!(!entities.contains(&entity2));
        assert!(entities.contains(&entity3));

        let Some((position, velocity)) = results.get(entity1) else {
            panic!("Entity 1 not found");
        };
        assert_eq!(*position, Position { x: 0.0, y: 0.0 });
        assert_eq!(*velocity, Velocity { x: 1.0, y: 1.0 });

        assert!(results.get(entity2).is_none());

        let Some((position, velocity)) = results.get(entity3) else {
            panic!("Entity 3 not found");
        };
        assert_eq!(*position, Position { x: 0.0, y: 0.0 });
        assert_eq!(*velocity, Velocity { x: 1.0, y: 1.0 });
    }
}
