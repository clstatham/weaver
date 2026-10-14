//! Derive macros for `weaver-ecs`. Re-exported from there, so depend on that crate instead.

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

/// Derives `SystemStage` for a label type.
///
/// The type needs `Clone + Eq + Hash + Debug + Send + Sync + 'static`; unit structs and fieldless
/// enums are the usual choice. The generated `dyn_hash` mixes in the type's `TypeId`, so two
/// different types with equal contents are still different stages. `SystemStage` has to be in
/// scope where the derive is used (it is in `weaver_ecs::prelude`), and the generated code names
/// `weaver_util` directly, so the deriving crate needs `weaver-util` as a dependency too.
///
/// # Examples
///
/// ```ignore
/// use weaver_ecs::prelude::*;
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
/// struct Update;
/// ```
#[proc_macro_derive(SystemStage)]
pub fn derive_system_stage(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut where_clause = where_clause.cloned().unwrap_or_else(|| syn::WhereClause {
        where_token: Default::default(),
        predicates: Default::default(),
    });
    where_clause
        .predicates
        .push(syn::parse2(quote! { Self: 'static + Send + Sync + Clone + Eq + ::core::fmt::Debug + ::core::hash::Hash }).unwrap());
    let expanded = quote! {
        impl #impl_generics SystemStage for #name #ty_generics #where_clause {
            fn dyn_clone(&self) -> Box<dyn SystemStage> {
                Box::new(::std::clone::Clone::clone(self))
            }

            fn as_dyn_eq(&self) -> &dyn weaver_util::label::DynEq {
                self
            }

            fn dyn_hash(&self, mut state: &mut dyn ::core::hash::Hasher) {
                let type_id = ::std::any::TypeId::of::<Self>();
                ::core::hash::Hash::hash(&type_id, &mut state);
                ::core::hash::Hash::hash(self, &mut state);
            }
        }
    };
    TokenStream::from(expanded)
}
