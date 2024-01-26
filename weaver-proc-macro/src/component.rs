use quote::{format_ident, quote};

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
        let name = method.path.get_ident().unwrap();
        let method = match method.value {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit_str),
                ..
            }) => lit_str.parse::<syn::TypeBareFn>().unwrap(),
            _ => panic!("Methods must be strings"),
        };
        let ret_ty = &method.output;
        let ret_ty = match ret_ty {
            syn::ReturnType::Default => quote! { None },
            syn::ReturnType::Type(_, ty) => match &**ty {
                syn::Type::Reference(syn::TypeReference {
                    elem, mutability, ..
                }) => {
                    if mutability.is_some() {
                        quote! { Some(weaver_ecs::component::MethodArgType::Mut(registry.get_static::<#elem>())) }
                    } else {
                        quote! { Some(weaver_ecs::component::MethodArgType::Ref(registry.get_static::<#elem>())) }
                    }
                }
                _ => {
                    quote! { Some(weaver_ecs::component::MethodArgType::Owned(registry.get_static::<#ty>())) }
                }
            },
        };
        let arg_muta_refs = method
            .inputs
            .iter()
            .map(|arg| {
                let mut is_reference = false;
                let muta = match arg.ty {
                    syn::Type::Reference(syn::TypeReference { mutability, .. }) => {
                        is_reference = true;
                        mutability.is_some()
                    }
                    _ => false,
                };
                (muta, is_reference)
            })
            .collect::<Vec<_>>();
        let arg_tys = method
            .inputs
            .iter()
            .map(|arg| {
                let ty = &arg.ty;
                let ty = match ty {
                    syn::Type::Reference(syn::TypeReference { elem, .. }) => elem,
                    _ => ty,
                };
                quote! { #ty }
            })
            .collect::<Vec<_>>();

        let args = method
            .inputs
            .iter()
            .enumerate()
            .map(|(i, arg)| {
                let name = arg
                    .name
                    .as_ref()
                    .map(|name| name.to_owned().0)
                    .unwrap_or(format_ident!("arg{i}"))
                    .to_owned();
                quote! { #name }
            })
            .collect::<Vec<_>>();

        let mut arg_bindings = quote! {};
        let mut arg_ty_bindings = vec![];
        for ((arg, arg_ty), (muta, is_ref)) in
            args.iter().zip(arg_tys.iter()).zip(arg_muta_refs.iter())
        {
            let arg_get_ty_id = quote! {
                registry.get_static::<#arg_ty>()
            };
            if *muta {
                arg_ty_bindings
                    .push(quote! { weaver_ecs::component::MethodArgType::Mut(#arg_get_ty_id) });
                arg_bindings.extend(
                    quote! { let mut #arg = &mut *args.next().unwrap().get_as_mut::<#arg_ty>().unwrap(); },
                );
            } else if *is_ref {
                arg_ty_bindings
                    .push(quote! { weaver_ecs::component::MethodArgType::Ref(#arg_get_ty_id) });
                arg_bindings.extend(
                    quote! { let #arg = &args.next().unwrap().get_as::<#arg_ty>().unwrap(); },
                );
            } else {
                arg_ty_bindings
                    .push(quote! { weaver_ecs::component::MethodArgType::Owned(#arg_get_ty_id) });
                arg_bindings.extend(
                    quote! { let #arg = *args.next().unwrap().get_as::<#arg_ty>().unwrap(); },
                );
            }
        }

        let gen = quote! {
            let registry_clone = registry.clone();
            methods.push(weaver_ecs::component::MethodWrapper::from_method(
                stringify!(#name),
                [#(#arg_ty_bindings),*],
                #ret_ty,
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
        syn::Data::Enum(_) => Default::default(),
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

            fn register_vtable(registry: &std::sync::Arc<weaver_ecs::registry::Registry>) {
                let mut methods = vec![];
                #method_gen
                let id = registry.get_static::<Self>();
                registry.register_vtable(id, methods);
            }
        }
    };
    gen.into()
}
