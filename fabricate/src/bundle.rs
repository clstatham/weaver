use crate::{
    prelude::Atom,
    registry::{Entity, StaticId},
    storage::Data,
};

/// A collection of components to be added to an entity.
pub trait Bundle: Sized + 'static {
    fn type_ids() -> Vec<Entity>;
    fn into_data_vec(self) -> Vec<Data>;
}

macro_rules! impl_bundle_for_tuple {
    ($($name:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($name: Atom),*> Bundle for ($($name,)*) {
            fn type_ids() -> Vec<Entity> {
                vec![$(<$name as StaticId>::static_type_uid()),*]
            }

            fn into_data_vec(self) -> Vec<Data> {
                let ($($name,)*) = self;
                vec![$(<$name as Atom>::into_data($name),)*]
            }
        }
    };
}

impl_bundle_for_tuple!(A);
impl_bundle_for_tuple!(A, B);
impl_bundle_for_tuple!(A, B, C);
impl_bundle_for_tuple!(A, B, C, D);
impl_bundle_for_tuple!(A, B, C, D, E);
impl_bundle_for_tuple!(A, B, C, D, E, F);
impl_bundle_for_tuple!(A, B, C, D, E, F, G);
impl_bundle_for_tuple!(A, B, C, D, E, F, G, H);
