use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::DeriveInput;

use crate::reflect_module;

pub fn derive_reflect(input: DeriveInput) -> Result<TokenStream, syn::Error> {
    let name = &input.ident;

    let mut reflected_traits = Vec::new();
    for attr in &input.attrs {
        if attr.path().is_ident("reflect") {
            let meta = &attr.meta;
            match meta {
                syn::Meta::List(list) => {
                    list.parse_nested_meta(|nested_meta| {
                        if let Some(ident) = nested_meta.path.get_ident() {
                            // reflected_traits.push(format_ident!("Reflect{}", ident));
                            reflected_traits.push(ident.clone());
                            Ok(())
                        } else {
                            Err(syn::Error::new_spanned(meta, "Expected an identifier"))
                        }
                    })?;
                }
                _ => {
                    return Err(syn::Error::new_spanned(meta, "Expected a list of traits"));
                }
            }
        }
    }

    let expanded = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => {
                let mut relected_fields = fields.clone().named.into_iter().collect::<Vec<_>>();
                // ignore #[reflect(ignore)] fields
                relected_fields.retain(|field| {
                    !field.attrs.iter().any(|attr| {
                        attr.path().is_ident("reflect")
                            && attr.to_token_stream().to_string().contains("ignore")
                    })
                });
                let reflected_fields = syn::FieldsNamed {
                    brace_token: fields.brace_token,
                    named: syn::punctuated::Punctuated::from_iter(relected_fields),
                };

                let reflect_impl = impl_reflect_struct(
                    name,
                    &reflected_fields,
                    &input.generics,
                    &reflected_traits,
                );
                let struct_impl = impl_struct(name, &reflected_fields, &input.generics);
                quote! {
                    #reflect_impl
                    #struct_impl
                }
            }
            syn::Fields::Unit => {
                let fields = syn::FieldsNamed {
                    brace_token: syn::token::Brace::default(),
                    named: syn::punctuated::Punctuated::new(),
                };
                let reflect_impl =
                    impl_reflect_struct(name, &fields, &input.generics, &reflected_traits);
                let struct_impl = impl_struct(name, &fields, &input.generics);
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

pub fn reflect_trait(input: &syn::ItemTrait) -> TokenStream {
    let reflect_module = reflect_module();

    let trait_name = &input.ident;
    let trait_vis = &input.vis;

    let reflect_trait_name = syn::Ident::new(&format!("Reflect{}", trait_name), trait_name.span());

    quote! {
        #input

        #trait_vis struct #reflect_trait_name {
            get_func: fn(&dyn #reflect_module::Reflect) -> Option<&dyn #trait_name>,
            get_mut_func: fn(&mut dyn #reflect_module::Reflect) -> Option<&mut dyn #trait_name>,
        }

        impl Clone for #reflect_trait_name {
            fn clone(&self) -> Self {
                Self {
                    get_func: self.get_func,
                    get_mut_func: self.get_mut_func,
                }
            }
        }

        impl #reflect_trait_name {
            pub fn get<'a>(&self, reflect: &'a dyn #reflect_module::Reflect) -> Option<&'a dyn #trait_name> {
                (self.get_func)(reflect)
            }

            pub fn get_mut<'a>(&self, reflect: &'a mut dyn #reflect_module::Reflect) -> Option<&'a mut dyn #trait_name> {
                (self.get_mut_func)(reflect)
            }
        }

        impl<T: #trait_name + #reflect_module::Reflect> #reflect_module::registry::FromType<T> for #reflect_trait_name {
            fn from_type() -> Self {
                Self {
                    get_func: |reflect| {
                        <dyn #reflect_module::Reflect>::downcast_ref::<T>(reflect).map(|t| t as &dyn #trait_name)
                    },
                    get_mut_func: |reflect| {
                        <dyn #reflect_module::Reflect>::downcast_mut::<T>(reflect).map(|t| t as &mut dyn #trait_name)
                    },
                }
            }
        }
    }
}

pub fn impl_reflect_struct(
    name: &syn::Ident,
    fields: &syn::FieldsNamed,
    generics: &syn::Generics,
    reflected_traits: &[syn::Ident],
) -> TokenStream {
    let field_names = fields.named.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        quote! { #field_name }
    });

    let field_types = fields.named.iter().map(|field| {
        let field_type = &field.ty;
        quote! { #field_type }
    });

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let reflect_module = reflect_module();

    quote! {
        impl #impl_generics #reflect_module::registry::Typed for #name #ty_generics #where_clause {
            fn type_name() -> &'static str {
                stringify!(#name)
            }

            fn type_info() -> &'static #reflect_module::registry::TypeInfo {
                static TYPE_INFO: std::sync::OnceLock<#reflect_module::registry::TypeInfo> = std::sync::OnceLock::new();
                TYPE_INFO.get_or_init(|| {
                    #reflect_module::registry::TypeInfo::Struct(#reflect_module::registry::StructInfo::new::<#name #ty_generics>(&[
                        #(
                            #reflect_module::registry::FieldInfo {
                                name: stringify!(#field_names),
                                type_id: std::any::TypeId::of::<#field_types>(),
                                type_name: std::any::type_name::<#field_types>(),
                            }
                        ),*
                    ]))
                })
            }

            fn get_type_registration() -> #reflect_module::registry::TypeRegistration {
                let mut reg = #reflect_module::registry::TypeRegistration {
                    type_id: std::any::TypeId::of::<#name #ty_generics>(),
                    type_name: Self::type_name(),
                    type_info: Self::type_info(),
                    type_aux_data: #reflect_module::registry::TypeIdMap::default(),
                };

                #(
                    reg.type_aux_data.insert(std::any::TypeId::of::<#reflected_traits>(), std::sync::Arc::new(<#reflected_traits as #reflect_module::registry::FromType<#name #ty_generics>>::from_type()));
                )*

                reg
            }
        }

        impl #impl_generics #reflect_module::registry::FromReflect for #name #ty_generics #where_clause {
            fn from_reflect(value: &dyn #reflect_module::Reflect) -> Option<&Self> {
                if let Some(value) = value.downcast_ref::<Self>() {
                    Some(value)
                } else {
                    None
                }
            }

            fn from_reflect_mut(value: &mut dyn #reflect_module::Reflect) -> Option<&mut Self> {
                if let Some(value) = value.downcast_mut::<Self>() {
                    Some(value)
                } else {
                    None
                }
            }
        }
    }
}

pub fn impl_struct(
    name: &syn::Ident,
    fields: &syn::FieldsNamed,
    generics: &syn::Generics,
) -> TokenStream {
    let field_names = fields
        .named
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            quote! { #field_name }
        })
        .collect::<Vec<_>>();

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let reflect_module = reflect_module();

    quote! {
        impl #impl_generics #reflect_module::registry::Struct for #name #ty_generics #where_clause {
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
