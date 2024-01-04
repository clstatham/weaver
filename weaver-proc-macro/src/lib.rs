use proc_macro::*;
use quote::{format_ident, quote};

static COMPONENT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static RESOURCE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[proc_macro_derive(Component)]
pub fn component_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_component_macro(&ast)
}

fn impl_component_macro(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let id = COMPONENT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let gen = quote! {
        unsafe impl crate::ecs::component::Component for #name {
            fn component_id() -> u64 {
                #id
            }
        }
    };
    gen.into()
}

#[proc_macro_derive(Resource)]
pub fn resource_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_resource_macro(&ast)
}

fn impl_resource_macro(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let id = RESOURCE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let gen = quote! {
        impl crate::ecs::resource::Resource for #name {
            fn resource_id() -> u64 {
                #id
            }
        }
    };
    gen.into()
}

#[proc_macro_attribute]
pub fn system(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ast = syn::parse(item).unwrap();
    impl_system_macro(attr, &ast)
}

fn impl_system_macro(attr: TokenStream, ast: &syn::ItemFn) -> TokenStream {
    let vis = &ast.vis;
    // get the name from the first attr
    let system_struct_name = syn::parse::<syn::Ident>(attr).unwrap();
    let args = &ast.sig.inputs;

    let mut query_types = Vec::new();
    let mut res_types = Vec::new();
    let mut resmut_types = Vec::new();

    let mut query_names = Vec::new();
    let mut res_names = Vec::new();
    let mut resmut_names = Vec::new();

    let mut seen_types = Vec::new();

    let mut commands_binding = None;

    // populate the above Vecs and do some compiletime checks to make sure the queries are valid
    // 1. no conflicting writes
    // 2. no conflicting reads
    // 3. arguments should all be Query<_>, Res<_>, ResMut<_>, Commands
    // 4. Queries can take &T or &mut T
    // 5. only one Commands argument is allowed
    // examples:
    // fn system(query: Query<&A>) {}
    // fn system(query: Query<&A>, query2: Query<&B>) {}
    // fn system(query: Query<&A>, res: Res<B>) {}
    // fn system(query: Query<&A>, res: ResMut<B>) {}
    // fn system(query: Query<&A>, res: ResMut<B>, res2: ResMut<C>) {}
    // fn system(query: Query<&A>, res: ResMut<B>, res2: ResMut<C>, query2: Query<&mut B>) {}
    // etc...

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
                                let inner_ty = match &path.segments.last().unwrap().arguments {
                                    syn::PathArguments::AngleBracketed(args) => {
                                        let inner_ty = &args.args[0];
                                        match inner_ty {
                                            syn::GenericArgument::Type(ty) => ty,
                                            _ => {
                                                panic!("Invalid argument type: Expected Query<...>")
                                            }
                                        }
                                    }
                                    _ => panic!("Invalid argument type: Expected Query<...>"),
                                };
                                match &inner_ty {
                                    syn::Type::Tuple(tuple) => {
                                        query_types.push(quote! {
                                            #tuple
                                        });
                                    }
                                    syn::Type::Reference(reference) => {
                                        let inner_ty = &reference.elem;
                                        match inner_ty.as_ref() {
                                            syn::Type::Path(path) => {
                                                let path = &path.path;
                                                let path_ident =
                                                    &path.segments.last().unwrap().ident;
                                                let muta = reference.mutability.is_some();
                                                if muta {
                                                    query_types.push(quote! { &mut #path_ident });
                                                } else {
                                                    query_types.push(quote! { &#path_ident });
                                                }
                                                if seen_types.contains(&path_ident.to_string()) {
                                                    panic!(
                                                        "Conflicting queries: {} is already being queried",
                                                        path_ident
                                                    )
                                                }
                                                seen_types.push(path_ident.to_string());
                                            }
                                            _ => panic!("Invalid argument type"),
                                        }
                                    }
                                    _ => {
                                        panic!("Invalid argument type: Expected Query<...>")
                                    }
                                }
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
                                if commands_binding.is_some() {
                                    panic!("Only one Commands argument is allowed")
                                }
                                commands_binding = Some(outer_pat.clone());
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
        Some(commands) => quote! {
            let mut #commands = Commands::new(&world);
        },
        None => quote! {},
    };

    let gen = quote! {
        #[allow(non_camel_case_types, dead_code)]
        #vis struct #system_struct_name;

        impl crate::ecs::system::System for #system_struct_name {
            #[allow(unused_mut)]
            fn run(&self, world: &crate::ecs::World) -> anyhow::Result<()> {
                #(
                    let mut #query_names = world.query::<#query_types>();
                )*
                #(
                    let #res_names = world.read_resource::<#res_types>()?;
                )*
                #(
                    let mut #resmut_names = world.write_resource::<#resmut_types>()?;
                )*
                #commands
                {
                    #body
                }
                Ok(())
            }

            fn components_read(&self) -> Vec<u64> {
                use crate::ecs::query::QueryFilter;
                let mut components = Vec::new();
                #(
                    components.extend_from_slice(&<#query_types>::reads().unwrap_or_default().into_iter().collect::<Vec<_>>());
                )*
                components
            }

            fn components_written(&self) -> Vec<u64> {
                use crate::ecs::query::QueryFilter;
                let mut components = Vec::new();
                #(
                    components.extend_from_slice(&<#query_types>::writes().unwrap_or_default().into_iter().collect::<Vec<_>>());
                )*
                components
            }
        }
    };
    gen.into()
}

#[proc_macro]
pub fn impl_bundle_for_tuple(input: TokenStream) -> TokenStream {
    let mut types = Vec::new();
    let mut names = Vec::new();
    for (i, arg) in syn::parse::<syn::TypeTuple>(input)
        .unwrap()
        .elems
        .into_iter()
        .enumerate()
    {
        // let name = syn::Index::from(i);
        let name = format_ident!("t{}", i);
        types.push(arg);
        names.push(name);
    }

    let gen = quote! {
        impl<#(#names),*> Bundle for (#(#names),*)
        where
            #(#names: Component),*
        {
            fn build(self, world: &World) -> anyhow::Result<Entity> {
                let (#(#names),*) = self;
                let entity = world.create_entity();
                #(
                    world.add_component(entity, #names)?;
                )*
                Ok(entity)
            }
        }
    };
    gen.into()
}

#[proc_macro_derive(Bundle)]
pub fn bundle_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_bundle_macro(&ast)
}

fn impl_bundle_macro(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let fields = match &ast.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => &fields.named,
            _ => panic!("Invalid struct"),
        },
        _ => panic!("Invalid struct"),
    };
    let fields = fields.iter().map(|field| {
        let name = &field.ident;
        quote! {
            #name
        }
    });
    let gen = quote! {
        impl crate::ecs::Bundle for #name {
            fn build(self, world: &crate::ecs::World) -> anyhow::Result<crate::ecs::Entity> {
                let entity = world.create_entity();
                #(
                    world.add_component(entity, self.#fields)?;
                )*
                Ok(entity)
            }
        }
    };
    gen.into()
}

#[proc_macro]
pub fn impl_query_for_n_tuple(input: TokenStream) -> TokenStream {
    let mut names = Vec::new();
    let n = syn::parse::<syn::LitInt>(input)
        .unwrap()
        .base10_parse::<usize>()
        .unwrap();
    for i in 0..n {
        let name = format_ident!("t{}", i);
        names.push(name);
    }

    let gen = quote! {
        impl<'a, #(#names),*> crate::ecs::query::QueryFilter<'a> for (#(#names),*)
        where
            #(#names: crate::ecs::query::QueryFilter<'a>,)*
            #(#names::Item: crate::ecs::Component,)*
        {
            type Item = (#(#names::Item),*);
            type ItemRef = (#(#names::ItemRef),*);

            fn get(entity: Entity, entries: &'a [QueryEntry]) -> Option<Self::ItemRef> {
                #(
                    let #names = #names::get(entity, entries)?;
                )*
                Some((#(#names),*))
            }

            fn reads() -> Option<crate::ecs::query::FxHashSet<u64>> {
                let mut reads = crate::ecs::query::FxHashSet::default();
                #(
                    reads.extend(&#names::reads().unwrap_or_default().into_iter().collect::<Vec<_>>());
                )*
                Some(reads)
            }

            fn writes() -> Option<crate::ecs::query::FxHashSet<u64>> {
                let mut writes = crate::ecs::query::FxHashSet::default();
                #(
                    writes.extend(&#names::writes().unwrap_or_default().into_iter().collect::<Vec<_>>());
                )*
                Some(writes)
            }
        }
    };

    gen.into()
}
