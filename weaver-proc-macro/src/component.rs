use quote::quote;

pub fn derive_component(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;

    let fields = match &ast.data {
        syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(syn::FieldsNamed { named, .. }),
            ..
        }) => named
            .iter()
            .filter(|field| matches!(field.vis, syn::Visibility::Public(_)))
            .collect::<Vec<_>>(),
        syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Unit,
            ..
        }) => Default::default(),
        _ => panic!("Component must be a struct with named fields"),
    };

    let field_names = fields
        .iter()
        .map(|field| field.ident.as_ref().unwrap())
        .collect::<Vec<_>>();

    let gen = quote! {
        impl weaver_ecs::component::Component for #name {
            fn fields(&self, registry: &std::sync::Arc<weaver_ecs::id::Registry>) -> Vec<weaver_ecs::component::Data> {
                vec![
                    #(weaver_ecs::component::Data::new(self.#field_names.clone(), Some(stringify!(#field_names)), registry)),*
                ]
            }
        }
    };
    gen.into()
}
