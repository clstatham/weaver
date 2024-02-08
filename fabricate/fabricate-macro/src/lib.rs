use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{punctuated::Punctuated, *};

enum TakesSelf {
    None,
    Ref,
    RefMut,
}

impl quote::ToTokens for TakesSelf {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            TakesSelf::None => quote!(fabricate::component::TakesSelf::None).to_tokens(tokens),
            TakesSelf::Ref => quote!(fabricate::component::TakesSelf::Ref).to_tokens(tokens),
            TakesSelf::RefMut => quote!(fabricate::component::TakesSelf::RefMut).to_tokens(tokens),
        }
    }
}

struct Method {
    name: Ident,
    args: Vec<Type>,
    ret: Type,
    takes_self: TakesSelf,
}

impl syn::parse::Parse for Method {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        let args;
        parenthesized!(args in input);
        let args = Punctuated::<Type, Token![,]>::parse_terminated(&args)?;
        let args: Vec<Type> = args.into_iter().collect();
        input.parse::<Token![->]>()?;
        let ret = input.parse()?;
        let takes_self = if args.is_empty() {
            TakesSelf::None
        } else if let Type::Reference(ref arg) = args[0] {
            if arg.elem.to_token_stream().to_string() == "Self" {
                if arg.mutability.is_none() {
                    TakesSelf::Ref
                } else {
                    TakesSelf::RefMut
                }
            } else {
                TakesSelf::None
            }
        } else if let Type::Path(ref path) = args[0] {
            if path.path.is_ident("Self") {
                TakesSelf::Ref
            } else {
                TakesSelf::None
            }
        } else {
            TakesSelf::None
        };
        Ok(Method {
            name,
            args,
            ret,
            takes_self,
        })
    }
}

#[proc_macro_derive(Atom, attributes(script_vtable, inspect))]
pub fn derive_atom(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let mut methods = Vec::new();

    for attr in &input.attrs {
        if attr.path().is_ident("script_vtable") {
            let ms = attr
                .parse_args_with(Punctuated::<Method, Token![,]>::parse_terminated)
                .unwrap();
            methods.extend(ms);
        }
    }

    let mut method_gen = Vec::new();

    for method in methods {
        let method_name = method.name;
        let arg_names = method
            .args
            .iter()
            .enumerate()
            .map(|(i, _)| format_ident!("arg{}", i))
            .collect::<Vec<_>>();
        let arg_bindings = method
            .args
            .iter()
            .zip(arg_names.iter())
            .map(|(arg_ty, arg_name)| {
                if let Type::Reference(syn::TypeReference {
                    mutability: Some(_),
                    elem,
                    ..
                }) = arg_ty
                {
                    quote! {
                        let mut #arg_name = #arg_name.as_mut::<#elem>().unwrap();
                    }
                } else if let Type::Reference(syn::TypeReference {
                    mutability: None,
                    elem,
                    ..
                }) = arg_ty
                {
                    quote! {
                        let #arg_name = #arg_name.as_ref::<#elem>().unwrap();
                    }
                } else if let Type::Path(syn::TypePath { path, .. }) = arg_ty {
                    quote! {
                        let #arg_name = #arg_name.into_owned::<#path>().unwrap();
                    }
                } else {
                    panic!("Unsupported arg type");
                }
            })
            .collect::<Vec<_>>();
        let arg_tys_id = method
            .args
            .iter()
            .map(|ty| {
                quote! {
                    <#ty as fabricate::registry::StaticId>::static_type_uid()
                }
            })
            .collect::<Vec<_>>();
        let ret = method.ret;
        let takes_self = method.takes_self;

        method_gen.push(quote! {
            map.insert(stringify!(#method_name).to_string(), fabricate::component::ScriptMethod {
                name: stringify!(#method_name).to_string(),
                args: vec![#(#arg_tys_id),*],
                ret: <#ret as fabricate::registry::StaticId>::static_type_uid(),
                takes_self: #takes_self,
                run: |mut args| {
                    let [#(#arg_names),*] = &mut args[..] else { fabricate::prelude::bail!("Wrong number of args") };
                    #(#arg_bindings)*
                    let ret = Self::#method_name(#(#arg_names),*);
                    Ok(vec![fabricate::storage::Data::new_dynamic(ret)])
                },
            });
        });
    }

    let mut inspect = quote! { vec![] };

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

            inspect = quote! {
                vec![
                    #(
                        <#field_types as fabricate::component::Atom>::as_value_ref(&mut self.#field_names, stringify!(#field_names)),
                    )*
                ]
            };
        }
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    quote! {
        impl #impl_generics fabricate::component::Atom for #name #ty_generics #where_clause {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
            fn as_any_box(self: Box<Self>) -> Box<dyn std::any::Any> {
                self
            }
            fn clone_box(&self) -> Box<dyn fabricate::component::Atom> {
                Box::new(self.clone())
            }

            fn script_vtable(&self) -> fabricate::component::ScriptVtable {
                let mut map = std::collections::HashMap::default();
                #(#method_gen)*
                fabricate::component::ScriptVtable { methods: map }
            }

            fn inspect(&mut self) -> Vec<fabricate::component::ValueRef> {
                #inspect
            }
        }
    }
    .into()
}
