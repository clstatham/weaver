use crate::{component::Data, storage::Components, TypeInfo};

use super::{Component, Entity};

/// A collection of components that can be built and added to an entity.
pub trait Bundle: Sized + Send + Sync + 'static {
    fn build(self, components: &mut Components) -> Entity {
        components.build(self)
    }
    fn component_infos() -> Vec<TypeInfo>;
    fn components(self) -> Vec<Data>;
}

impl Bundle for () {
    fn build(self, components: &mut Components) -> Entity {
        components.create_entity()
    }
    fn component_infos() -> Vec<TypeInfo> {
        Vec::new()
    }
    fn components(self) -> Vec<Data> {
        Vec::new()
    }
}

impl<T: Component> Bundle for T {
    fn component_infos() -> Vec<TypeInfo> {
        vec![TypeInfo::of::<T>()]
    }
    fn components(self) -> Vec<Data> {
        vec![Data::new(self)]
    }
}

impl<A: Bundle> Bundle for (A,) {
    fn component_infos() -> Vec<TypeInfo> {
        A::component_infos()
    }
    fn components(self) -> Vec<Data> {
        let (a,) = self;
        a.components()
    }
}

macro_rules! impl_bundle_for_tuple {
    (($($name:ident),*)) => {
        #[allow(non_snake_case)]
        impl<$($name: Bundle),*> Bundle for ($($name,)*) {
            fn component_infos() -> Vec<TypeInfo> {
                let mut infos = vec![$($name::component_infos()),*].concat();
                infos.sort_by_key(|info| info.id);
                infos
            }
            fn components(self) -> Vec<Data> {
                let ($($name,)*) = self;
                let mut comps = vec![];
                $(comps.extend($name.components());)*
                comps.sort_by_key(|comp| comp.id());
                comps
            }
        }
    };
}

impl_bundle_for_tuple!((A, B));
impl_bundle_for_tuple!((A, B, C));
impl_bundle_for_tuple!((A, B, C, D));
impl_bundle_for_tuple!((A, B, C, D, E));
impl_bundle_for_tuple!((A, B, C, D, E, F));
impl_bundle_for_tuple!((A, B, C, D, E, F, G));
impl_bundle_for_tuple!((A, B, C, D, E, F, G, H));
