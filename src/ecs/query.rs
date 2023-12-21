use super::{component::Component, world::World};

/// A read-only query. This is used to get immutable references to components found in a `World`.
pub struct Read<'a> {
    pub components: Vec<&'a dyn Component>,
}

/// A read-write query. This is used to get mutable references to components found in a `World`.
pub struct Write<'a> {
    pub components: Vec<&'a mut dyn Component>,
}

impl<'a> Read<'a> {
    /// Returns a vector of references to components of type `T` found with the query.
    pub fn get<T: Component>(&self) -> Vec<&T> {
        let mut result = Vec::new();
        for component in self.components.iter() {
            if let Some(component) = component.as_any().downcast_ref::<T>() {
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
            if let Some(component) = component.as_any().downcast_ref::<T>() {
                result.push(component);
            }
        }
        result
    }

    /// Returns a vector of mutable references to components of type `T` found with the query.
    pub fn get_mut<T: Component>(&mut self) -> Vec<&mut T> {
        let mut result = Vec::new();
        for component in self.components.iter_mut() {
            if let Some(component) = component.as_any_mut().downcast_mut::<T>() {
                result.push(component);
            }
        }
        result
    }
}

pub trait MultiComponent {
    fn read<'a, 'b: 'a>(world: &'b World) -> Read<'a>;
    fn write<'a, 'b: 'a>(world: &'b mut World) -> Write<'a>;
}

macro_rules! impl_multi_component_for_tuple {
    ($head:ident, $($tail:ident,)*) => {
        #[allow(unused_variables, non_snake_case)]
        impl<$head: Component, $($tail: Component,)*> MultiComponent for ($head, $($tail),*)
        {
            fn read<'a, 'b: 'a>(world: &'b World) -> Read<'a> {
                let mut result = Vec::new();
                let mut $head = false;
                $(let mut $tail = false;)*
                'outer: for (_entity, components) in world.components.data.iter() {
                    let mut temp = Vec::new();
                    for comp in components.iter() {
                        if let Some(_) = comp.as_any().downcast_ref::<$head>() {
                            temp.push(&**comp);
                            $head = true;
                            continue;
                        }

                        $(
                            if let Some(_) = comp.as_any().downcast_ref::<$tail>() {
                                temp.push(&**comp);
                                $tail = true;
                                continue;
                            }
                        )*
                    }

                    if $head $(&& $tail)* {
                        result.extend(temp);
                    }
                }

                Read { components: result }
            }

            fn write<'a, 'b: 'a>(world: &'b mut World) -> Write<'a> {
                let mut result = Vec::new();

                'outer: for (_entity, components) in world.components.data.iter_mut() {
                    let mut temp = Vec::new();
                    let mut $head = false;
                    $(let mut $tail = false;)*
                    for comp in components.iter_mut() {
                        if let Some(_) = comp.as_any_mut().downcast_mut::<$head>() {
                            temp.push(&mut **comp);
                            $head = true;
                            continue;
                        }

                        $(
                            if let Some(_) = comp.as_any_mut().downcast_mut::<$tail>() {
                                temp.push(&mut **comp);
                                $tail = true;
                                continue;
                            }
                        )*
                    }

                    if $head $(&& $tail)* {
                        result.extend(temp);
                    }
                }

                Write { components: result }
            }
        }
    };
}

impl_multi_component_for_tuple!(A,);
impl_multi_component_for_tuple!(A, B,);
impl_multi_component_for_tuple!(A, B, C,);
impl_multi_component_for_tuple!(A, B, C, D,);
impl_multi_component_for_tuple!(A, B, C, D, E,);
impl_multi_component_for_tuple!(A, B, C, D, E, F,);
