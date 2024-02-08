use proc_macro::TokenStream;
use quote::quote;

pub fn derive_inspect(input: &syn::DeriveInput) -> TokenStream {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut get_values = quote! { vec![] };
    let mut value_names = quote! { vec![] };
    let mut value = quote! { None };

    if let syn::Data::Struct(data) = &input.data {
        if let syn::Fields::Named(fields) = &data.fields {
            let fields = fields
                .named
                .iter()
                .filter(|field| {
                    field
                        .attrs
                        .iter()
                        .any(|attr| attr.path().is_ident("inspect"))
                })
                .collect::<Vec<_>>();
            let field_types = fields.iter().map(|field| &field.ty).collect::<Vec<_>>();
            let field_names = fields.iter().map(|field| &field.ident).collect::<Vec<_>>();

            get_values = quote! {
                vec![
                    #(
                        <#field_types as weaver_core::ecs_ext::inspect::Value>::as_value_ref(&mut self.#field_names, stringify!(#field_names)),
                    )*
                ]
            };

            value_names = quote! {
                vec![#(stringify!(#field_names)),*]
            };

            value = quote! {
                match name {
                    #(
                        stringify!(#field_names) => Some(<#field_types as weaver_core::ecs_ext::inspect::Value>::as_value_ref(&mut self.#field_names, stringify!(#field_names))),
                    )*
                    _ => None,
                }
            };
        }
    }

    quote! {
        impl #impl_generics weaver_core::ecs_ext::inspect::Inspect for #name #ty_generics #where_clause {
            fn get_values(&mut self) -> Vec<weaver_core::ecs_ext::inspect::ValueRef> {
                #get_values
            }

            fn value_names(&self) -> Vec<&'static str> {
                #value_names
            }

            fn value(&mut self, name: &str) -> Option<weaver_core::ecs_ext::inspect::ValueRef> {
                #value
            }
        }
    }
    .into()
}
