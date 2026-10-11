use std::any::TypeId;

use weaver_util::prelude::*;

use crate::{
    change_detection::{ComponentTicks, Tick},
    component::{Component, ComponentVec},
};

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
                    vec.push($name);
                    vec
                }),*]
            }

            fn from_components(mut components: Vec<ComponentVec>) -> Result<Box<Self>> {
                // look each component up by type, since bundles store them sorted by TypeId
                let result = ($(
                    match components.iter().position(|c| c.element_typeid() == TypeId::of::<$name>()) {
                        Some(index) => match components.swap_remove(index).pop::<$name>() {
                            Some(component) => component,
                            None => bail!("Component vec for {} is empty", std::any::type_name::<$name>()),
                        },
                        None => bail!("Expected component of type {}, found none", std::any::type_name::<$name>()),
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
    pub(crate) ticks: Vec<ComponentTicks>,
}

impl ComponentBundle {
    pub fn from_tuple<T: Bundle>(bundle: T, tick: Tick) -> Self {
        let mut types = T::component_type_ids();
        let mut components = T::into_components(bundle);

        types.sort_unstable();
        components.sort_unstable_by_key(|component| component.element_typeid());

        let mut ticks = Vec::new();
        for _ in &components {
            ticks.push(ComponentTicks::new(tick));
        }

        Self {
            types,
            components,
            ticks,
        }
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

    pub fn insert(
        &mut self,
        mut comp: ComponentVec,
        mut ticks: ComponentTicks,
    ) -> Option<(ComponentVec, ComponentTicks)> {
        for (i, t) in self.types.iter().copied().enumerate() {
            #[allow(clippy::comparison_chain)]
            if t == comp.element_typeid() {
                std::mem::swap(&mut self.components[i], &mut comp);
                std::mem::swap(&mut self.ticks[i], &mut ticks);
                return Some((comp, ticks));
            } else if t > comp.element_typeid() {
                self.types.insert(i, comp.element_typeid());
                self.components.insert(i, comp);
                self.ticks.insert(i, ticks);
                return None;
            }
        }

        self.types.push(comp.element_typeid());
        self.components.push(comp);
        self.ticks.push(ticks);
        None
    }

    pub fn remove<T: Component>(&mut self) -> Option<T> {
        let ty = TypeId::of::<T>();
        let index = self.types.iter().position(|t| *t == ty)?;
        self.types.remove(index);
        self.ticks.remove(index);
        self.components.remove(index).pop::<T>()
    }

    /// Merges an entity's existing components into this (newer) bundle. Where both have a
    /// component of the same type, this bundle's value wins, but it keeps the existing `added`
    /// tick so replacing a component reads as a change rather than an addition.
    pub fn merge_existing(&mut self, existing: Self) {
        for ((ty, component), ticks) in existing
            .types
            .into_iter()
            .zip(existing.components)
            .zip(existing.ticks)
        {
            match self.types.binary_search(&ty) {
                Ok(index) => self.ticks[index].added = ticks.added,
                Err(index) => {
                    self.types.insert(index, ty);
                    self.components.insert(index, component);
                    self.ticks.insert(index, ticks);
                }
            }
        }
    }

    pub fn union(&mut self, other: Self) -> Option<Self> {
        let mut ret = Self {
            types: Vec::new(),
            components: Vec::new(),
            ticks: Vec::new(),
        };
        for (v, t) in other.components.into_iter().zip(other.ticks) {
            if let Some((comp, ticks)) = self.insert(v, t) {
                ret.insert(comp, ticks);
            }
        }

        if ret.types.is_empty() {
            None
        } else {
            Some(ret)
        }
    }
}
