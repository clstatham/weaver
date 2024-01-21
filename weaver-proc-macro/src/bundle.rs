use quote::quote;

pub fn derive_bundle(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;
    let fields = match &ast.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => &fields.named,
            _ => panic!("Invalid struct"),
        },
        _ => panic!("Invalid struct"),
    };
    let field_names = fields
        .iter()
        .map(|field| {
            let name = &field.ident;
            quote! {
                #name
            }
        })
        .collect::<Vec<_>>();
    let field_types = fields
        .clone()
        .into_iter()
        .map(|field| {
            let ty = &field.ty;
            quote! {
                #ty
            }
        })
        .collect::<Vec<_>>();
    let gen = quote! {
        impl weaver_ecs::bundle::Bundle for #name {
            fn component_types(registry: &weaver_ecs::id::IdRegistry) -> Vec<weaver_ecs::id::DynamicId> {
                let mut infos = Vec::new();
                #(
                    infos.push(registry.get_static::<#field_types>());
                )*
                infos.sort_unstable();
                infos
            }
            fn components(self, registry: &weaver_ecs::id::IdRegistry) -> Vec<weaver_ecs::component::Data> {
                let mut components = Vec::new();
                #(
                    components.push(weaver_ecs::component::Data::new(self.#field_names, registry));
                )*
                components.sort_unstable_by_key(|ptr| ptr.id());
                components
            }
        }
    };
    gen.into()
}
