use std::cell::{Ref, RefMut};

use super::{component::Component, entity::Entity, world::World};

/// A read-only query. This is used to get immutable references to components found in a `World`.
pub struct Read<'a> {
    pub components: Vec<Ref<'a, Box<dyn Component>>>,
}

/// A read-write query. This is used to get mutable references to components found in a `World`.
pub struct Write<'a> {
    pub components: Vec<RefMut<'a, Box<dyn Component>>>,
}

impl<'a> Read<'a> {
    /// Returns a vector of references to components of type `T` found with the query.
    pub fn get<T: Component>(&self) -> Vec<&T> {
        let mut result = Vec::new();
        for component in self.components.iter() {
            if let Some(component) = component.as_ref().as_any().downcast_ref::<T>() {
                result.push(component);
            }
        }
        result
    }
}

impl<'a> Write<'a> {
    /// Returns a vector of references to components of type `T` found with the query.
    pub fn get<T: Component>(&self) -> Vec<&T> {
        let mut result = Vec::new();
        for component in self.components.iter() {
            if let Some(component) = component.as_ref().as_any().downcast_ref::<T>() {
                result.push(component);
            }
        }
        result
    }

    /// Returns a vector of mutable references to components of type `T` found with the query.
    pub fn get_mut<T: Component>(&mut self) -> Vec<&mut T> {
        let mut result = Vec::new();
        for component in self.components.iter_mut() {
            if let Some(component) = component.as_mut().as_any_mut().downcast_mut::<T>() {
                result.push(component);
            }
        }
        result
    }
}

pub trait Query
where
    Self: 'static,
{
    fn query<'a, 'b: 'a>(world: &'b World) -> Vec<(Entity, usize)>;
}

macro_rules! impl_query_for_tuple {
    ($head:ident, $($tail:ident,)*) => {
        #[allow(unused_variables, non_snake_case, unused_mut)]
        impl<$head: Query, $($tail: Query,)*> Query for ($head, $($tail),*)
        {
            /// Logical "AND" of all queries. Returns the entities that are in all queries, and the indices of the matching components for each query.
            fn query<'a, 'b: 'a>(world: &'b World) -> Vec<(Entity, usize)> {
                let $head = $head::query(world);
                $(let $tail = $tail::query(world);)*

                let mut result = $head.clone();

                // For each entity in the head query, check if it exists in all other queries and remove it if it doesn't.
                for (entity, index) in $head.iter() {
                    $(
                        if !$tail.iter().any(|(e, _)| e == entity) {
                            result.retain(|(e, _)| e != entity);
                        }
                    )*
                }

                // For each entity in all the head and tail queries, add the indices to the result.
                let temp = result.clone();
                for (entity, index) in temp.iter() {
                    $(
                        if let Some((_, i)) = $tail.iter().find(|(e, _)| e == entity) {
                            if !result.iter().any(|(e, _)| e == entity && i == index) {
                                result.push((*entity, *i));
                            }
                        }
                    )*
                }

                result
            }
        }
    };
}

impl_query_for_tuple!(A,);
impl_query_for_tuple!(A, B,);
impl_query_for_tuple!(A, B, C,);
impl_query_for_tuple!(A, B, C, D,);
impl_query_for_tuple!(A, B, C, D, E,);
impl_query_for_tuple!(A, B, C, D, E, F,);

#[allow(unused_variables, non_snake_case, unused_mut)]
impl<T: Component> Query for T {
    fn query<'a, 'b: 'a>(world: &'b World) -> Vec<(Entity, usize)> {
        let mut result = Vec::new();
        for (entity, components) in world.components.data.iter() {
            let mut temp = Vec::new();
            for (i, comp) in components.iter().enumerate() {
                let comp = comp.borrow();
                if let Some(t) = comp.as_ref().as_any().downcast_ref::<T>() {
                    temp.push((*entity, i));
                }
            }
            result.extend(temp);
        }
        result
    }
}

/// A query that returns all entities that do NOT have a component of type `T`.
pub struct Without<T>(std::marker::PhantomData<T>);
// impl<T: Component> Component for Without<T> {}

#[allow(unused_variables, non_snake_case, unused_mut)]
impl<T: Component> Query for Without<T> {
    fn query<'a, 'b: 'a>(world: &'b World) -> Vec<(Entity, usize)> {
        let mut result = Vec::new();
        'outer: for (entity, components) in world.components.data.iter() {
            let mut temp = Vec::new();
            for (i, comp) in components.iter().enumerate() {
                let comp = comp.borrow();
                if let Some(t) = comp.as_ref().as_any().downcast_ref::<T>() {
                    continue 'outer;
                }
                temp.push((*entity, i));
            }
            result.extend(temp);
        }
        result
    }
}
