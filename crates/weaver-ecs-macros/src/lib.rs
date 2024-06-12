use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

#[proc_macro_derive(Component)]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.ident;

    let expanded = quote! {
        impl #impl_generics weaver_ecs::prelude::Component for #name #ty_generics #where_clause {
        }
    };

    expanded.into()
}

#[proc_macro_derive(Resource)]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.ident;

    let expanded = quote! {
        impl #impl_generics weaver_ecs::prelude::Resource for #name #ty_generics #where_clause {
        }
    };

    expanded.into()
}
