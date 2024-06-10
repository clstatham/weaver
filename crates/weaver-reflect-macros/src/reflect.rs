use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

use crate::reflect_module;

pub fn derive_reflect(input: DeriveInput) -> Result<TokenStream, syn::Error> {
    let name = &input.ident;
    // let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => {
                let reflect_impl = impl_reflect_struct(name, fields);
                let struct_impl = impl_struct(name, fields);
                quote! {
                    #reflect_impl
                    #struct_impl
                }
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    name,
                    "Only named fields are supported",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(name, "Only structs are supported"));
        }
    };

    Ok(expanded)
}

pub fn impl_reflect_struct(name: &syn::Ident, fields: &syn::FieldsNamed) -> TokenStream {
    let field_names = fields.named.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        quote! { #field_name }
    });

    let field_types = fields.named.iter().map(|field| {
        let field_type = &field.ty;
        quote! { #field_type }
    });

    let reflect_module = reflect_module();

    quote! {
        impl #reflect_module::Reflect for #name {
            fn as_reflect(&self) -> &dyn #reflect_module::Reflect {
                self
            }

            fn as_reflect_mut(&mut self) -> &mut dyn #reflect_module::Reflect {
                self
            }

            fn into_reflect_box(self: Box<Self>) -> Box<dyn #reflect_module::Reflect> {
                self
            }

            fn reflect_type_name(&self) -> &'static str {
                <Self as #reflect_module::Typed>::type_name()
            }
        }

        impl #reflect_module::Typed for #name {
            fn type_name() -> &'static str {
                stringify!(#name)
            }

            fn type_info() -> &'static #reflect_module::TypeInfo {
                static TYPE_INFO: std::sync::OnceLock<#reflect_module::TypeInfo> = std::sync::OnceLock::new();
                TYPE_INFO.get_or_init(|| {
                    #reflect_module::TypeInfo::Struct(#reflect_module::StructInfo::new::<#name>(&[
                        #(
                            #reflect_module::FieldInfo {
                                name: stringify!(#field_names),
                                type_id: std::any::TypeId::of::<#field_types>(),
                                type_name: std::any::type_name::<#field_types>(),
                            }
                        ),*
                    ]))
                })
            }
        }
    }
}

pub fn impl_struct(name: &syn::Ident, fields: &syn::FieldsNamed) -> TokenStream {
    let field_names = fields
        .named
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            quote! { #field_name }
        })
        .collect::<Vec<_>>();

    let reflect_module = reflect_module();

    quote! {
        impl #reflect_module::Struct for #name {
            fn field(&self, field_name: &str) -> Option<&dyn #reflect_module::Reflect> {
                match field_name {
                    #(
                        stringify!(#field_names) => Some(&self.#field_names),
                    )*
                    _ => None,
                }
            }

            fn field_mut(&mut self, field_name: &str) -> Option<&mut dyn #reflect_module::Reflect> {
                match field_name {
                    #(
                        stringify!(#field_names) => Some(&mut self.#field_names),
                    )*
                    _ => None,
                }
            }
        }
    }
}
