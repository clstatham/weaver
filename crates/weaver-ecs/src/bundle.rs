use weaver_util::{impl_downcast, DowncastSync};

use crate::prelude::Component;

pub trait Bundle: DowncastSync {
    fn into_components(self) -> Vec<Box<dyn Component>>;
}
impl_downcast!(Bundle);

impl<T: Component> Bundle for T {
    fn into_components(self) -> Vec<Box<dyn Component>> {
        vec![Box::new(self)]
    }
}

impl Bundle for Vec<Box<dyn Component>> {
    fn into_components(self) -> Vec<Box<dyn Component>> {
        self
    }
}

impl Bundle for Box<dyn Component> {
    fn into_components(self) -> Vec<Box<dyn Component>> {
        vec![self]
    }
}

impl<T: Component> Bundle for Vec<T> {
    fn into_components(self) -> Vec<Box<dyn Component>> {
        self.into_iter()
            .map(|c| Box::new(c) as Box<dyn Component>)
            .collect()
    }
}

macro_rules! impl_bundle_tuple {
    ($($name:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($name: Component),*> Bundle for ($($name,)*) {
            fn into_components(self) -> Vec<Box<dyn Component>> {
                let ($($name,)*) = self;
                vec![$(Box::new($name) as Box<dyn Component>,)*]
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
