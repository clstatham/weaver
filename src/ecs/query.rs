use super::{component::Component, world::World};

pub struct ReadResult<'a> {
    pub components: Vec<&'a dyn Component>,
}

pub struct WriteResult<'a> {
    pub components: Vec<&'a mut dyn Component>,
}

impl<'a> ReadResult<'a> {
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

impl<'a> WriteResult<'a> {
    pub fn get<T: Component>(&self) -> Vec<&T> {
        let mut result = Vec::new();
        for component in self.components.iter() {
            if let Some(component) = component.as_any().downcast_ref::<T>() {
                result.push(component);
            }
        }
        result
    }

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
    fn read<'a, 'b: 'a>(world: &'b World) -> ReadResult<'a>;
    fn write<'a, 'b: 'a>(world: &'b mut World) -> WriteResult<'a>;
}

macro_rules! impl_multi_component_for_tuple {
    ($head:ident, $($tail:ident,)*) => {
        #[allow(unused_variables, non_snake_case)]
        impl<$head: Component, $($tail: Component,)*> MultiComponent for ($head, $($tail),*)
        {
            fn read<'a, 'b: 'a>(world: &'b World) -> ReadResult<'a> {
                let mut result = Vec::new();

                'outer: for (_entity, components) in world.components.data.iter() {
                    let mut temp = Vec::new();
                    for comp in components.iter() {
                        if let Some(_) = comp.as_any().downcast_ref::<$head>() {
                            temp.push(&**comp);
                            continue;
                        }

                        $(
                            if let Some(_) = comp.as_any().downcast_ref::<$tail>() {
                                temp.push(&**comp);
                                continue;
                            }
                        )*

                        // doesn't contain the components we need
                        continue 'outer;
                    }

                    result.extend(temp);
                }

                ReadResult { components: result }
            }

            fn write<'a, 'b: 'a>(world: &'b mut World) -> WriteResult<'a> {
                let mut result = Vec::new();

                'outer: for (_entity, components) in world.components.data.iter_mut() {
                    let mut temp = Vec::new();
                    for comp in components.iter_mut() {
                        if let Some(_) = comp.as_any_mut().downcast_mut::<$head>() {
                            temp.push(&mut **comp);
                            continue;
                        }

                        $(
                            if let Some(_) = comp.as_any_mut().downcast_mut::<$tail>() {
                                temp.push(&mut **comp);
                                continue;
                            }
                        )*

                        // doesn't contain the components we need
                        continue 'outer;
                    }

                    result.extend(temp);
                }

                WriteResult { components: result }
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
