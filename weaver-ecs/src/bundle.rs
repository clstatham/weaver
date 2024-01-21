use weaver_proc_macro::all_tuples;

use crate::{
    component::{Component, Data},
    id::{DynamicId, IdRegistry},
    storage::Components,
};

use super::entity::Entity;

/// A collection of components that can be built and added to an entity.
pub trait Bundle: Sized + Send + Sync + 'static {
    fn build(self, components: &mut Components) -> Entity {
        components.build(self)
    }
    fn component_types(registry: &IdRegistry) -> Vec<DynamicId>;
    fn components(self, registry: &IdRegistry) -> Vec<Data>;
}

impl Bundle for () {
    fn build(self, components: &mut Components) -> Entity {
        components.create_entity()
    }
    fn component_types(_registry: &IdRegistry) -> Vec<DynamicId> {
        Vec::new()
    }
    fn components(self, _registry: &IdRegistry) -> Vec<Data> {
        Vec::new()
    }
}

impl<T: Component> Bundle for T {
    fn component_types(registry: &IdRegistry) -> Vec<DynamicId> {
        vec![registry.get_static::<T>()]
    }
    fn components(self, registry: &IdRegistry) -> Vec<Data> {
        vec![Data::new(self, registry)]
    }
}

macro_rules! impl_bundle_for_tuple {
    ($($name:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($name: Bundle),*> Bundle for ($($name,)*) {
            fn component_types(registry: &IdRegistry) -> Vec<DynamicId> {
                let mut infos = vec![$($name::component_types(registry)),*].concat();
                infos.sort_unstable();
                infos
            }
            fn components(self, registry: &IdRegistry) -> Vec<Data> {
                let ($($name,)*) = self;
                let mut comps = vec![];
                $(comps.extend($name.components(registry));)*
                comps.sort_unstable_by_key(|comp| comp.id());
                comps
            }
        }
    };
}

all_tuples!(1..=16, impl_bundle_for_tuple);
