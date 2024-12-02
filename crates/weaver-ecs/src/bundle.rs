use std::any::{Any, TypeId};

use any_vec::any_value::{AnyValue, AnyValueWrapper};
use weaver_util::prelude::*;

use crate::component::{Component, ComponentVec};

pub trait Bundle: Send + Sync + 'static {
    fn component_type_ids() -> Vec<TypeId>;
    fn empty_vecs() -> Vec<ComponentVec>;
    fn into_components(self) -> Vec<ComponentVec>;
    fn from_components(components: Vec<ComponentVec>) -> Result<Box<Self>>;
}

macro_rules! impl_bundle_tuple {
    ($($name:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($name: Component),*> Bundle for ($($name,)*) {
            fn component_type_ids() -> Vec<TypeId> {
                vec![$(TypeId::of::<$name>(),)*]
            }

            fn empty_vecs() -> Vec<ComponentVec> {
                vec![$(ComponentVec::new::<$name>(),)*]
            }

            fn into_components(self) -> Vec<ComponentVec> {
                let ($($name,)*) = self;
                vec![$({
                    let mut vec = ComponentVec::new::<$name>();
                    vec.push(AnyValueWrapper::new($name));
                    vec
                }),*]
            }

            fn from_components(mut components: Vec<ComponentVec>) -> Result<Box<Self>> {
                let result = ($(
                    match components.pop() {
                        Some(mut component) => {
                            if component.element_typeid() == TypeId::of::<$name>() {
                                component.pop().unwrap().downcast::<$name>().unwrap()
                            } else {
                                bail!("Expected component of type {:?}, found {:?}", TypeId::of::<$name>(), component.type_id())
                            }
                        }
                        None => bail!("Expected component of type {:?}, found none", TypeId::of::<$name>())
                    },
                )*);

                Ok(Box::new(result))
            }
        }
    };
}

impl_bundle_tuple!(A);
impl_bundle_tuple!(A, B);
impl_bundle_tuple!(A, B, C);
impl_bundle_tuple!(A, B, C, D);
impl_bundle_tuple!(A, B, C, D, E);
impl_bundle_tuple!(A, B, C, D, E, F);
impl_bundle_tuple!(A, B, C, D, E, F, G);
impl_bundle_tuple!(A, B, C, D, E, F, G, H);

pub struct ComponentBundle {
    pub(crate) types: Vec<TypeId>,
    pub(crate) components: Vec<ComponentVec>,
}

impl ComponentBundle {
    pub fn from_tuple<T: Bundle>(bundle: T) -> Self {
        let mut types = T::component_type_ids();
        let mut components = T::into_components(bundle);

        types.sort_unstable();
        components.sort_unstable_by_key(|component| component.element_typeid());

        Self { types, components }
    }

    pub fn into_tuple<T: Bundle>(self) -> Result<T> {
        Ok(*T::from_components(self.components)?)
    }

    pub fn types(&self) -> &[TypeId] {
        &self.types
    }

    pub fn components(&self) -> &[ComponentVec] {
        &self.components
    }

    pub fn empty_vecs(&self) -> Vec<ComponentVec> {
        self.components
            .iter()
            .map(|component| component.clone_empty())
            .collect()
    }

    pub fn insert(&mut self, mut comp: ComponentVec) -> Option<ComponentVec> {
        for (i, t) in self.types.iter().copied().enumerate() {
            #[allow(clippy::comparison_chain)]
            if t == comp.element_typeid() {
                std::mem::swap(&mut self.components[i], &mut comp);
                return Some(comp);
            } else if t > comp.element_typeid() {
                self.types.insert(i, comp.element_typeid());
                self.components.insert(i, comp);
                return None;
            }
        }

        self.types.push(comp.element_typeid());
        self.components.push(comp);
        None
    }

    pub fn remove<T: Component>(&mut self) -> Option<T> {
        let ty = TypeId::of::<T>();
        let index = self.types.iter().position(|t| *t == ty)?;
        self.types.remove(index);
        let mut comp = self.components.remove(index);
        let comp = comp.pop().unwrap();
        Some(comp.downcast().unwrap())
    }

    pub fn union(&mut self, other: Self) -> Option<Self> {
        let mut ret = Self {
            types: Vec::new(),
            components: Vec::new(),
        };
        for v in other.components.into_iter() {
            if let Some(comp) = self.insert(v) {
                ret.insert(comp);
            }
        }

        if ret.types.is_empty() {
            None
        } else {
            Some(ret)
        }
    }
}
