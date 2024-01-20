use quote::quote;

pub fn derive_component(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;

    let gen = quote! {
        impl weaver_ecs::component::Component for #name {}
    };
    gen.into()
}
