use std::{
    any::Any,
    hash::{Hash, Hasher},
};

pub trait DynEq: Any {
    fn as_any(&self) -> &dyn Any;
    fn dyn_eq(&self, other: &dyn DynEq) -> bool;
}

impl<T: Any + Eq> DynEq for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn dyn_eq(&self, other: &dyn DynEq) -> bool {
        if let Some(other) = other.as_any().downcast_ref::<T>() {
            self == other
        } else {
            false
        }
    }
}

pub trait DynHash: Any {
    fn as_dyn_eq(&self) -> &dyn DynEq;
    fn dyn_hash<H: Hasher>(&self, state: &mut H);
}

impl<T: Any + Eq + Hash> DynHash for T {
    fn as_dyn_eq(&self) -> &dyn DynEq {
        self
    }

    fn dyn_hash<H: Hasher>(&self, state: &mut H) {
        self.hash(state);
        self.type_id().hash(state);
    }
}

// yoinked directly from bevy's source code, with minor changes
// https://github.com/bevyengine/bevy/blob/793e2f2000e43d212a76e81aefa09223e5d65bb1/crates/bevy_ecs/src/label.rs#L94
#[macro_export]
macro_rules! define_label {
    (
        $(#[$label_attr:meta])*
        $label_trait_name:ident,
        $interner_name:ident
    ) => {
        $crate::define_label!(
            $(#[$label_attr])*
            $label_trait_name,
            $interner_name,
            extra_methods: {},
            extra_methods_impl: {}
        );
    };
    (
        $(#[$label_attr:meta])*
        $label_trait_name:ident,
        $interner_name:ident,
        extra_methods: { $($trait_extra_methods:tt)* },
        extra_methods_impl: { $($interned_extra_methods_impl:tt)* }
    ) => {

        $(#[$label_attr])*
        pub trait $label_trait_name: 'static + Send + Sync + ::core::fmt::Debug {

            $($trait_extra_methods)*

            /// Clones this `
            #[doc = stringify!($label_trait_name)]
            ///`.
            fn dyn_clone(&self) -> Box<dyn $label_trait_name>;

            /// Casts this value to a form where it can be compared with other type-erased values.
            fn as_dyn_eq(&self) -> &dyn $crate::label::DynEq;

            /// Feeds this value into the given [`Hasher`].
            fn dyn_hash(&self, state: &mut dyn ::core::hash::Hasher);

            /// Returns an [`Interned`] value corresponding to `self`.
            fn intern(&self) -> $crate::intern::Interned<dyn $label_trait_name>
            where Self: Sized {
                $interner_name.intern(self)
            }
        }

        impl $label_trait_name for $crate::intern::Interned<dyn $label_trait_name> {

            $($interned_extra_methods_impl)*

            fn dyn_clone(&self) -> Box<dyn $label_trait_name> {
                (**self).dyn_clone()
            }

            /// Casts this value to a form where it can be compared with other type-erased values.
            fn as_dyn_eq(&self) -> &dyn $crate::label::DynEq {
                (**self).as_dyn_eq()
            }

            fn dyn_hash(&self, state: &mut dyn ::core::hash::Hasher) {
                (**self).dyn_hash(state);
            }

            fn intern(&self) -> Self {
                *self
            }
        }

        impl PartialEq for dyn $label_trait_name {
            fn eq(&self, other: &Self) -> bool {
                self.as_dyn_eq().dyn_eq(other.as_dyn_eq())
            }
        }

        impl Eq for dyn $label_trait_name {}

        impl ::core::hash::Hash for dyn $label_trait_name {
            fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                self.dyn_hash(state);
            }
        }

        impl $crate::intern::Internable for dyn $label_trait_name {
            fn leak(&self) -> &'static Self {
                Box::leak(self.dyn_clone())
            }

            fn ref_eq(&self, other: &Self) -> bool {
                use ::core::ptr;

                // Test that both the type id and pointer address are equivalent.
                self.as_dyn_eq().type_id() == other.as_dyn_eq().type_id()
                    && ptr::addr_eq(ptr::from_ref::<Self>(self), ptr::from_ref::<Self>(other))
            }

            fn ref_hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                use ::core::{hash::Hash, ptr};

                // Hash the type id...
                self.as_dyn_eq().type_id().hash(state);

                // ...and the pointer address.
                // Cast to a unit `()` first to discard any pointer metadata.
                ptr::from_ref::<Self>(self).cast::<()>().hash(state);
            }
        }

        static $interner_name: $crate::intern::Interner<dyn $label_trait_name> =
            $crate::intern::Interner::new();
    };
}
