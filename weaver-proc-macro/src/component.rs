use quote::quote;

pub fn derive_component(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;
    let attrs = &ast.attrs;

    let methods = attrs.iter().filter_map(|attr| {
        if attr.path().is_ident("method") {
            Some(attr.parse_args::<syn::MetaNameValue>().unwrap())
        } else {
            None
        }
    });

    let mut method_gen = quote! {};

    for method in methods {
        let method = match method.value {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit_str),
                ..
            }) => lit_str.parse::<syn::ItemFn>().unwrap(),
            _ => panic!("Methods must be strings"),
        };
        let name = &method.sig.ident;
        let args = method
            .sig
            .inputs
            .iter()
            .map(|arg| {
                let name = match arg {
                    syn::FnArg::Receiver(_) => panic!("Methods cannot have receivers"),
                    syn::FnArg::Typed(pat) => match &*pat.pat {
                        syn::Pat::Ident(ident) => &ident.ident,
                        _ => panic!("Methods must have named arguments"),
                    },
                };
                quote! { #name }
            })
            .collect::<Vec<_>>();
        let arg_tys = method
            .sig
            .inputs
            .iter()
            .map(|arg| {
                let ty = match arg {
                    syn::FnArg::Receiver(_) => panic!("Methods cannot have receivers"),
                    syn::FnArg::Typed(pat) => &pat.ty,
                };
                quote! { #ty }
            })
            .collect::<Vec<_>>();

        let mut arg_bindings = quote! {};
        for (arg, arg_ty) in args.iter().zip(arg_tys.iter()) {
            arg_bindings.extend(
                quote! { let mut #arg = &mut *args.next().unwrap().get_as_mut::<#arg_ty>(); },
            );
        }

        let num_args = args.len();

        let gen = quote! {
            let registry_clone = registry.clone();
            methods.push(weaver_ecs::component::MethodWrapper::from_method(
                stringify!(#name),
                #num_args,
                move |data: &[&weaver_ecs::component::Data]| {
                    let result = {
                        let mut args = data.iter();
                        #arg_bindings;
                        Self::#name(#(#args),*)
                    };
                    Ok(weaver_ecs::component::Data::new(result, None, &registry_clone))
                },
            ));
        };

        method_gen.extend(gen);
    }

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
            fn type_name() -> &'static str {
                stringify!(#name)
            }

            fn fields(&self, registry: &std::sync::Arc<weaver_ecs::registry::Registry>) -> Vec<weaver_ecs::component::Data> {
                vec![
                    #(weaver_ecs::component::Data::new(self.#field_names.clone(), Some(stringify!(#field_names)), registry)),*
                ]
            }

            fn register_methods(registry: &std::sync::Arc<weaver_ecs::registry::Registry>) {
                let mut methods = vec![];
                #method_gen
                let id = registry.get_static::<Self>();
                registry.register_methods(id, methods);
            }
        }
    };
    gen.into()
}
