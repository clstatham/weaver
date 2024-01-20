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
            fn component_infos() -> Vec<weaver_ecs::TypeInfo> {
                let mut infos = Vec::new();
                #(
                    infos.push(weaver_ecs::TypeInfo::of::<#field_types>());
                )*
                infos.sort_by_key(|info| info.id());
                infos
            }
            fn components(self) -> Vec<weaver_ecs::component::Data> {
                let mut components = Vec::new();
                #(
                    components.push(weaver_ecs::component::Data::new(self.#field_names));
                )*
                components.sort_by_key(|ptr| ptr.id());
                components
            }
        }
    };
    gen.into()
}
