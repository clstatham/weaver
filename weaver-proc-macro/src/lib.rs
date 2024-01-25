use std::ops::RangeInclusive;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{punctuated::Punctuated, Token};

mod bindable_component;
mod bundle;
mod component;
mod gpu_component;
mod system;

#[proc_macro_derive(Component, attributes(method))]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as syn::DeriveInput);
    component::derive_component(&ast)
}

#[proc_macro_derive(Bundle)]
pub fn derive_bundle(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as syn::DeriveInput);
    bundle::derive_bundle(&ast)
}

#[proc_macro_derive(GpuComponent, attributes(gpu))]
pub fn derive_gpu_component(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as syn::DeriveInput);
    gpu_component::derive_gpu_component(&ast)
}

#[proc_macro_derive(BindableComponent, attributes(uniform, texture, sampler, storage))]
pub fn derive_bindable_component(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as syn::DeriveInput);
    bindable_component::derive_bindable_component(&ast)
}

#[proc_macro_attribute]
pub fn system(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(item as syn::ItemFn);
    system::system(attr, &ast)
}

#[proc_macro]
pub fn big_tuple(input: TokenStream) -> TokenStream {
    let n = syn::parse_macro_input!(input as syn::LitInt);
    let n = n.base10_parse::<usize>().unwrap();

    let mut fields = vec![];
    for i in 1..=n {
        let name = format_ident!("T{}", i);
        fields.push(quote! { #name });
    }

    let fields: Punctuated<_, Token![,]> = fields.into_iter().collect();

    let gen = quote! {
        (#fields)
    };

    gen.into()
}

struct AllTuples {
    n: RangeInclusive<usize>,
    ident: syn::Ident,
}

impl syn::parse::Parse for AllTuples {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let start = input.parse::<syn::LitInt>()?.base10_parse::<usize>()?;
        input.parse::<syn::Token![..=]>()?;
        let end = input.parse::<syn::LitInt>()?.base10_parse::<usize>()?;
        input.parse::<syn::Token![,]>()?;
        let func = input.parse()?;
        Ok(Self {
            n: start..=end,
            ident: func,
        })
    }
}

#[proc_macro]
pub fn all_tuples(input: TokenStream) -> TokenStream {
    let AllTuples { n, ident: func } = syn::parse_macro_input!(input as AllTuples);

    let mut gen = quote! {};

    for i in n {
        let tuple: proc_macro2::TokenStream = big_tuple(quote! { #i }.into()).into();

        gen.extend(quote! {
            #func! #tuple;
        });
    }

    gen.into()
}
