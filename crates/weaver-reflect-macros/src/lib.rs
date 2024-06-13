use proc_macro::TokenStream;
use syn::{parse_macro_input, parse_quote};

mod reflect;

pub(crate) fn reflect_module() -> syn::Path {
    parse_quote!(weaver_ecs::reflect)
}

#[proc_macro_derive(Reflect, attributes(reflect))]
pub fn derive_reflect(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    reflect::derive_reflect(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn reflect_trait(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::ItemTrait);
    reflect::reflect_trait(&input).into()
}
