use quote::quote;

struct SystemArgs {
    name: syn::Ident,
    inputs: Vec<syn::FnArg>,
}

impl syn::parse::Parse for SystemArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        let inputs;
        syn::parenthesized!(inputs in input);
        let inputs = inputs
            .parse_terminated(syn::FnArg::parse, syn::Token![,])?
            .into_iter()
            .collect::<Vec<_>>();
        Ok(Self { name, inputs })
    }
}

pub fn system(attr: proc_macro::TokenStream, ast: &syn::ItemFn) -> proc_macro::TokenStream {
    let vis = &ast.vis;
    // get the name from the first attr
    let system_args = syn::parse::<SystemArgs>(attr).unwrap();
    let SystemArgs {
        name: system_struct_name,
        inputs,
    } = system_args;
    let args = &ast.sig.inputs;

    let mut query_types = Vec::new();
    let mut filter_types = Vec::new();
    let mut res_types = Vec::new();
    let mut resmut_types = Vec::new();

    let mut query_names = Vec::new();
    let mut res_names = Vec::new();
    let mut resmut_names = Vec::new();

    let mut seen_types = Vec::new();

    let mut commands_binding = None;

    // populate the above Vecs with the types and names of the arguments

    // examples of valid queries:
    //     fn foo(query: Query<&Position, Without<Velocity>>)
    //     fn foo(query: Query<&Position, ()>)
    //     fn foo(query: Query<&Position>)
    //     fn foo(query: Query<&Position, (Without<Velocity>, Without<Acceleration>)>)
    //     fn foo(query: Query<(&mut Position, &Velocity), Without<Acceleration>>)
    //     fn foo(query: Query<(&Position, &Velocity)>)

    // examples of valid resources:
    //     fn foo(res: Res<Position>)
    //     fn foo(res: ResMut<Position>)
    //     fn foo(res: Res<Position>, res2: ResMut<Velocity>)
    //     fn foo(res: Res<Position>, res2: ResMut<Velocity>, res3: Res<Acceleration>)

    // any of the above can have a Commands argument mixed in as well

    for arg in args.iter() {
        match arg {
            syn::FnArg::Typed(outer) => {
                let outer_pat = &outer.pat;
                let outer_ty = &outer.ty;

                match outer_ty.as_ref() {
                    syn::Type::Path(path) => {
                        let path = &path.path;
                        let path_ident = path.segments.last().unwrap().ident.to_string();
                        match path_ident.as_str() {
                            "Query" => {
                                let query_name = match outer_pat.as_ref() {
                                    syn::Pat::Ident(ident) => ident.ident.clone(),
                                    _ => panic!("Invalid argument type"),
                                };
                                query_names.push(query_name.clone());

                                let seg = path.segments.last().unwrap();

                                match &seg.arguments {
                                    syn::PathArguments::AngleBracketed(args) => {
                                        let args =
                                            args.args.clone().into_iter().collect::<Vec<_>>();
                                        let query = &args[0];
                                        let filter = args.get(1);

                                        match query {
                                            syn::GenericArgument::Type(ty) => {
                                                query_types.push(ty.clone());
                                            }
                                            _ => panic!("Invalid argument type"),
                                        }

                                        match filter {
                                            Some(syn::GenericArgument::Type(ty)) => {
                                                filter_types.push(ty.clone());
                                            }
                                            None => {
                                                filter_types
                                                    .push(syn::parse2(quote! { () }).unwrap());
                                            }
                                            _ => panic!("Invalid argument type"),
                                        }
                                    }
                                    _ => panic!("Invalid argument type: Expected Query<...>"),
                                };
                            }
                            "Res" => {
                                let res_name = match outer_pat.as_ref() {
                                    syn::Pat::Ident(ident) => ident.ident.clone(),
                                    _ => panic!("Invalid argument type"),
                                };
                                res_names.push(res_name.clone());
                                let inner_ty = match &path.segments.last().unwrap().arguments {
                                    syn::PathArguments::AngleBracketed(args) => {
                                        let inner_ty = &args.args[0];
                                        match inner_ty {
                                            syn::GenericArgument::Type(ty) => ty,
                                            _ => panic!("Invalid argument type: Expected Res<...>"),
                                        }
                                    }
                                    _ => panic!("Invalid argument type: Expected Res<...>"),
                                };
                                match &inner_ty {
                                    syn::Type::Tuple(_tuple) => {
                                        panic!("Res cannot take a tuple as its inner type")
                                    }
                                    syn::Type::Path(path) => {
                                        let path = &path.path;
                                        let path_ident = &path.segments.last().unwrap().ident;
                                        res_types.push(path_ident.clone());
                                        if seen_types.contains(&path_ident.to_string()) {
                                            panic!(
                                                "Conflicting queries: {} is already being queried",
                                                path_ident
                                            )
                                        }
                                        seen_types.push(path_ident.to_string());
                                    }
                                    _ => {
                                        panic!("Invalid argument type: Expected Res<...>")
                                    }
                                }
                            }
                            "ResMut" => {
                                let resmut_name = match outer_pat.as_ref() {
                                    syn::Pat::Ident(ident) => ident.ident.clone(),
                                    _ => panic!("Invalid argument type"),
                                };
                                resmut_names.push(resmut_name.clone());
                                let inner_ty = match &path.segments.last().unwrap().arguments {
                                    syn::PathArguments::AngleBracketed(args) => {
                                        let inner_ty = &args.args[0];
                                        match inner_ty {
                                            syn::GenericArgument::Type(ty) => ty,
                                            _ => {
                                                panic!(
                                                    "Invalid argument type: Expected ResMut<...>"
                                                )
                                            }
                                        }
                                    }
                                    _ => panic!("Invalid argument type: Expected ResMut<...>"),
                                };
                                match &inner_ty {
                                    syn::Type::Tuple(_tuple) => {
                                        panic!("ResMut cannot take a tuple as its inner type")
                                    }
                                    syn::Type::Path(path) => {
                                        let path = &path.path;
                                        let path_ident = &path.segments.last().unwrap().ident;
                                        resmut_types.push(path_ident.clone());
                                        if seen_types.contains(&path_ident.to_string()) {
                                            panic!(
                                                "Conflicting queries: {} is already being queried",
                                                path_ident
                                            )
                                        }
                                        seen_types.push(path_ident.to_string());
                                    }
                                    _ => {
                                        panic!("Invalid argument type: Expected ResMut<...>")
                                    }
                                }
                            }
                            "Commands" => {
                                let commands_name = match outer_pat.as_ref() {
                                    syn::Pat::Ident(ident) => ident.ident.clone(),
                                    _ => panic!("Invalid argument type"),
                                };
                                commands_binding = Some(commands_name.clone());
                            }
                            _ => panic!(
                                "Invalid argument type: Expected one of `Query`, `Res`, or `ResMut`"
                            ),
                        }
                    }
                    _ => panic!("Invalid argument type"),
                }
            }
            _ => panic!("Invalid argument type"),
        }
    }

    let body = &ast.block;

    let commands = match commands_binding {
        Some(ref commands) => {
            quote! { let mut #commands = weaver_ecs::commands::Commands::new(world.clone()); }
        }
        None => quote! {},
    };

    let commands_finalize = match commands_binding {
        Some(commands) => quote! { {
            #commands.finalize(&mut world.write());
        } },
        None => quote! {},
    };

    let mut inputs_bindings = Vec::new();
    for (i, input) in inputs.iter().enumerate() {
        let input_name = match input {
            syn::FnArg::Typed(ty) => match ty.pat.as_ref() {
                syn::Pat::Ident(ident) => ident.ident.clone(),
                _ => panic!("Invalid argument type"),
            },
            _ => panic!("Invalid argument type"),
        };
        let input_ty = match input {
            syn::FnArg::Typed(ty) => &ty.ty,
            _ => panic!("Invalid argument type"),
        };
        let (input_ty, input_muta) = match &**input_ty {
            syn::Type::Reference(syn::TypeReference {
                elem, mutability, ..
            }) => (elem, mutability.is_some()),
            _ => (input_ty, false),
        };

        let binding = if input_muta {
            quote! { let mut #input_name = input[#i].get_as_mut::<#input_ty>().ok_or(anyhow::anyhow!("Invalid type for argument {}", #i)); }
        } else {
            quote! { let #input_name = input[#i].get_as::<#input_ty>().ok_or(anyhow::anyhow!("Invalid type for argument {}", #i)); }
        };
        inputs_bindings.push(binding);
    }

    let run_fn = quote! {
        fn run(&self, world: std::sync::Arc<weaver_ecs::prelude::RwLock<weaver_ecs::world::World>>, input: &[&weaver_ecs::component::Data]) -> anyhow::Result<()> {
            #(#inputs_bindings)*
            #commands
            {
                let world_lock = world.read();
                #(
                    let mut #query_names: Query<#query_types, #filter_types> = world_lock.query_filtered();
                )*
                #(
                    let #res_names = world_lock.read_resource::<#res_types>()?;
                )*
                #(
                    let mut #resmut_names = world_lock.write_resource::<#resmut_types>()?;
                )*

                #body
            }
            #commands_finalize
            Ok(())
        }
    };

    let gen = quote! {
        #[allow(non_camel_case_types, dead_code)]
        #vis struct #system_struct_name;

        impl weaver_ecs::system::System for #system_struct_name {
            #[allow(unused_mut)]
            #run_fn

            fn components_read(&self, registry: &weaver_ecs::registry::Registry) -> Vec<weaver_ecs::registry::DynamicId> {
                use weaver_ecs::query::Queryable;
                let mut components = Vec::new();
                #(
                    components.extend(<#query_types as Queryable<#filter_types>>::access(registry).reads.sparse_iter());
                )*
                components
            }

            fn components_written(&self, registry: &weaver_ecs::registry::Registry) -> Vec<weaver_ecs::registry::DynamicId> {
                use weaver_ecs::query::Queryable;
                let mut components = Vec::new();
                #(
                    components.extend(<#query_types as Queryable<#filter_types>>::access(registry).writes.sparse_iter());
                )*
                components
            }

            fn resources_read(&self, registry: &weaver_ecs::registry::Registry) -> Vec<weaver_ecs::registry::DynamicId> {
                let mut resources = Vec::new();
                #(
                    resources.push(registry.get_static::<#res_types>());
                )*
                resources
            }

            fn resources_written(&self, registry: &weaver_ecs::registry::Registry) -> Vec<weaver_ecs::registry::DynamicId> {
                let mut resources = Vec::new();
                #(
                    resources.push(registry.get_static::<#resmut_types>());
                )*
                resources
            }

            fn is_exclusive(&self) -> bool {
                todo!("System::is_exclusive")
            }
        }
    };
    gen.into()
}
