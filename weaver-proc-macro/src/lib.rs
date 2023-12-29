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
        unsafe impl weaver_ecs::component::Component for #name {
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
        impl weaver_ecs::resource::Resource for #name {
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

    // populate the above Vecs and do some compiletime checks to make sure the queries are valid
    // 1. no conflicting writes
    // 2. no conflicting reads
    // 3. arguments should all be Query<_>, Res<_>, or ResMut<_>
    // 4. Queries can take tuples as their inner types, but Res and ResMut cannot
    // examples:
    // #[system(A)]
    // fn system_a(a: Query<Read<CompA>>) {}
    // #[system(B)]
    // fn system_b(a: Query<Read<CompA>>, b: Query<Write<CompB>>) {}
    // #[system(C)]
    // fn system_c(a: Query<(Read<CompA>, Write<CompB>)>) {}
    // #[system(D)]
    // fn system_d(a: Query<Read<CompA>>, b: Res<ResA>, c: ResMut<ResB>) {}

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
                                        for inner_ty in &tuple.elems {
                                            match inner_ty {
                                                syn::Type::Path(path) => {
                                                    // make sure it's Read<T> or Write<T> and verify that the inner T has not yet been seen
                                                    let path = &path.path;
                                                    let path_ident =
                                                        &path.segments.last().unwrap().ident;
                                                    let path_ident = path_ident.to_string();
                                                    match path_ident.as_str() {
                                                        "Read" | "Write" => {
                                                            let inner_ty = match &path
                                                                .segments
                                                                .last()
                                                                .unwrap()
                                                                .arguments
                                                            {
                                                                syn::PathArguments::AngleBracketed(
                                                                    args,
                                                                ) => {
                                                                    let inner_ty = &args.args[0];
                                                                    match inner_ty {
                                                                        syn::GenericArgument::Type(
                                                                            ty,
                                                                        ) => ty,
                                                                        _ => panic!(
                                                                            "Invalid argument type: Expected Query<...>"
                                                                        ),
                                                                    }
                                                                }
                                                                _ => panic!("Invalid argument type: Expected Query<...>"),
                                                            };
                                                            let inner_ty = match inner_ty {
                                                                syn::Type::Path(path) => {
                                                                    let path = &path.path;
                                                                    let path_ident = &path
                                                                        .segments
                                                                        .last()
                                                                        .unwrap()
                                                                        .ident;
                                                                    path_ident.clone()
                                                                }
                                                                _ => panic!(
                                                                    "Invalid argument type: Expected Query<...>"
                                                                ),
                                                            };
                                                            if seen_types.contains(&inner_ty.to_string()) {
                                                                panic!(
                                                                    "Conflicting queries: {} is already being queried",
                                                                    inner_ty
                                                                )
                                                            }
                                                            seen_types.push(inner_ty.to_string());
                                                        }
                                                        _ => panic!(
                                                            "Invalid argument type: Expected Query<...>"
                                                        ),
                                                    }
                                                }
                                                _ => panic!(
                                                    "Invalid argument type: Expected Query<...>"
                                                ),
                                            }
                                        }
                                    }
                                    syn::Type::Path(path) => {
                                        let path = &path.path;
                                        let path_ident = &path.segments.last().unwrap().ident;

                                        match path_ident.to_string().as_str() {
                                            "Read" | "Write" => {
                                                let inner_ty = match &path
                                                    .segments
                                                    .last()
                                                    .unwrap()
                                                    .arguments
                                                {
                                                    syn::PathArguments::AngleBracketed(args) => {
                                                        let inner_ty = &args.args[0];
                                                        match inner_ty {
                                                            syn::GenericArgument::Type(ty) => ty,
                                                            _ => panic!(
                                                                "Invalid argument type: Expected Query<...>"
                                                            ),
                                                        }
                                                    }
                                                    _ => panic!("Invalid argument type: Expected Query<...>"),
                                                };
                                                let inner_ty = match inner_ty {
                                                    syn::Type::Path(path) => {
                                                        let path = &path.path;
                                                        let path_ident =
                                                            &path.segments.last().unwrap().ident;
                                                        path_ident.clone()
                                                    }
                                                    _ => panic!(
                                                        "Invalid argument type: Expected Query<...>"
                                                    ),
                                                };
                                                if seen_types.contains(&inner_ty.to_string()) {
                                                    panic!(
                                                        "Conflicting queries: {} is already being queried",
                                                        inner_ty
                                                    )
                                                }
                                                seen_types.push(inner_ty.to_string());
                                            }
                                            _ => {
                                                panic!("Invalid argument type: Expected Query<...>")
                                            }
                                        }
                                    }
                                    _ => panic!("Invalid argument type: Expected Query<...>"),
                                }

                                query_types.push(inner_ty.clone());
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

    let gen = quote! {
        #[allow(non_camel_case_types, dead_code)]
        #vis struct #system_struct_name;

        impl weaver_ecs::system::System for #system_struct_name {
            #[allow(unused_mut)]
            fn run(&self, world: &weaver_ecs::World) {
                #(
                    let mut #query_names = world.query::<#query_types>();
                )*
                #(
                    let #res_names = world.read_resource::<#res_types>();
                )*
                #(
                    let mut #resmut_names = world.write_resource::<#resmut_types>();
                )*
                {
                    #body
                }
            }

            fn components_read(&self) -> Vec<u64> {
                let mut components = Vec::new();
                #(
                    components.extend_from_slice(&<#query_types>::components_read());
                )*
                components
            }

            fn components_written(&self) -> Vec<u64> {
                let mut components = Vec::new();
                #(
                    components.extend_from_slice(&<#query_types>::components_written());
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
            fn build(self, world: &mut World) -> Entity {
                let (#(#names),*) = self;
                let entity = world.create_entity();
                #(
                    world.add_component(entity, #names);
                )*
                entity
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
        impl weaver_ecs::Bundle for #name {
            fn build(self, world: &mut weaver_ecs::World) -> weaver_ecs::Entity {
                let entity = world.create_entity();
                #(
                    world.add_component(entity, self.#fields);
                )*
                entity
            }
        }
    };
    gen.into()
}

#[proc_macro]
pub fn impl_queryable_for_n_tuple(input: TokenStream) -> TokenStream {
    let mut query_names = Vec::new();
    let mut item_names = Vec::new();
    let mut item_refs = Vec::new();
    let mut tuple_indices = Vec::new();

    let count = syn::parse::<syn::LitInt>(input.clone())
        .unwrap()
        .base10_parse::<usize>()
        .unwrap();

    for i in 0..count {
        let query_name = format_ident!("Q{}", i);
        query_names.push(query_name.clone());
        item_names.push(quote! {
            #query_name::Item
        });
        item_refs.push(quote! {
            #query_name::ItemRef
        });
        tuple_indices.push(syn::Index::from(i));
    }

    let first_query_name = &query_names[0];

    let rest_query_names = &query_names[1..];

    let gen = quote! {
        impl<'w, 'q, 'i, #(#query_names),*> Queryable<'w, 'q, 'i> for (#(#query_names),*)
        where
            'w: 'q,
            'q: 'i,
            #(#query_names: Queryable<'w, 'q, 'i>),*
        {
            type Item = (#(#item_names),*);
            type ItemRef = (#(#item_refs),*);
            type Iter = Box<dyn Iterator<Item = Self::ItemRef> + 'i>;

            fn create(world: &'w World) -> Self {
                (#(
                    #query_names::create(world)
                ),*)
            }

            fn entities(&self) -> BTreeSet<Entity> {
                let (#(#query_names),*) = self;
                let mut entities = #first_query_name.entities();
                #(
                    entities = entities.bitand(&#rest_query_names.entities());
                )*
                entities
            }

            fn components_read() -> Vec<u64>
            where
                Self: Sized,
            {
                let mut components = Vec::new();
                #(
                    components.extend_from_slice(&#query_names::components_read());
                )*
                components
            }

            fn components_written() -> Vec<u64>
            where
                Self: Sized,
            {
                let mut components = Vec::new();
                #(
                    components.extend_from_slice(&#query_names::components_written());
                )*
                components
            }

            fn get(&'q self, entity: Entity) -> Option<Self::ItemRef> {
                let entities = self.entities();
                let (#(#query_names),*) = self;
                if entities.contains(&entity) {
                    Some((
                        #first_query_name.get(entity)?,
                        #(
                            #rest_query_names.get(entity)?
                        ),*
                    ))
                } else {
                    None
                }
            }

            fn iter(&'q self) -> Self::Iter {
                let entities = self.entities();
                let (#(#query_names),*) = self;

                Box::new(
                    entities
                        .into_iter()
                        .map(|entity| (
                            #first_query_name.get(entity).unwrap(),
                            #(
                                #rest_query_names.get(entity).unwrap()
                            ),*
                        ))
                )
            }
        }
    };

    gen.into()
}
