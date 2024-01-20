use quote::quote;

pub fn derive_resource(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;

    let gen = quote! {
        impl weaver_ecs::resource::Resource for #name {}
    };
    gen.into()
}
