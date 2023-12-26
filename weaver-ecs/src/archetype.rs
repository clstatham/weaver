use crate::Component;

pub trait Archetype {}

macro_rules! impl_archetype_for_tuple {
    ($($name:ident),*) => {
        impl<$($name),*> Archetype for ($($name),*) where $($name: Component),* {}
    };
}

impl Archetype for () {}
impl<A> Archetype for (A,) where A: Component {}
impl_archetype_for_tuple!(A, B);
impl_archetype_for_tuple!(A, B, C);
impl_archetype_for_tuple!(A, B, C, D);
impl_archetype_for_tuple!(A, B, C, D, E);
impl_archetype_for_tuple!(A, B, C, D, E, F);
impl_archetype_for_tuple!(A, B, C, D, E, F, G);
impl_archetype_for_tuple!(A, B, C, D, E, F, G, H);
