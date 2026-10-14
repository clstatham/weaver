//! Bundles: groups of components that are spawned or inserted together.

use std::any::TypeId;

use weaver_util::prelude::*;

use crate::{
    change_detection::{ComponentTicks, Tick},
    component::{Component, ComponentVec},
};

/// A group of components that are inserted into an entity together.
///
/// Implemented for tuples of up to eight [`Component`]s, so `(Position, Velocity)` is a bundle and
/// so is the one-element `(Position,)`. The order inside the tuple doesn't matter to storage:
/// components are sorted by `TypeId` before they reach an archetype. A bundle should not name the
/// same component type twice.
pub trait Bundle: Send + Sync + 'static {
    /// The `TypeId` of each component, in tuple order.
    fn component_type_ids() -> Vec<TypeId>;
    /// One empty [`ComponentVec`] per component, in tuple order.
    fn empty_vecs() -> Vec<ComponentVec>;
    /// Splits the bundle into one single-element [`ComponentVec`] per component, in tuple order.
    fn into_components(self) -> Vec<ComponentVec>;
    /// Rebuilds the bundle from single-element vecs, found by element type (any order).
    ///
    /// # Errors
    ///
    /// Returns an error if a component type is missing from `components` or its vec is empty.
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

/// A type-erased bundle: the components of one entity, sorted by `TypeId`, with their change ticks.
///
/// This is the form an entity takes while it moves between archetypes. Each [`ComponentVec`]
/// holds exactly one element.
pub struct ComponentBundle {
    pub(crate) types: Vec<TypeId>,
    pub(crate) components: Vec<ComponentVec>,
    pub(crate) ticks: Vec<ComponentTicks>,
}

impl ComponentBundle {
    /// Erases a typed bundle, stamping every component as added and changed at `tick`.
    pub fn from_tuple<T: Bundle>(bundle: T, tick: Tick) -> Self {
        // FIXME: a bundle naming the same type twice keeps both entries, but lookups only ever find
        // one column, so a value is silently lost
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

    /// Converts back into a typed bundle.
    ///
    /// # Errors
    ///
    /// Returns an error if `T` needs a component this bundle doesn't have.
    pub fn into_tuple<T: Bundle>(self) -> Result<T> {
        Ok(*T::from_components(self.components)?)
    }

    /// The component types, sorted.
    pub fn types(&self) -> &[TypeId] {
        &self.types
    }

    /// The component values, in the same order as [`ComponentBundle::types`].
    pub fn components(&self) -> &[ComponentVec] {
        &self.components
    }

    /// Empty vecs of the same component types, for building an archetype's columns.
    pub fn empty_vecs(&self) -> Vec<ComponentVec> {
        self.components
            .iter()
            .map(|component| component.clone_empty())
            .collect()
    }

    /// Adds one component, keeping the types sorted. If the bundle already had that type, the old
    /// value and ticks are swapped out and returned.
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

    /// Removes and returns the `T` component, or `None` if the bundle has none.
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

    /// Moves every component of `other` into this bundle. Where both have a type, `other`'s value
    /// wins; the values it displaced come back as a bundle, or `None` if nothing collided.
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
