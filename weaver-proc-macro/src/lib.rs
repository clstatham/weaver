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

    let mut queries = Vec::new();

    let mut reads = Vec::new();
    let mut writes = Vec::new();

    // do some compiletime checks to make sure the queries are valid
    // 1. no conflicting writes
    // 2. no conflicting reads
    // 3. arguments should all be Query<_>
    for arg in args.iter() {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                let pat = &pat_type.pat;
                let ty = &pat_type.ty;
                match (&**pat, &**ty) {
                    (syn::Pat::Ident(_ident), syn::Type::Path(path)) => {
                        let ty_seg = &path.path.segments.last().unwrap();
                        if ty_seg.ident == "Query" {
                            match ty_seg.arguments {
                                syn::PathArguments::AngleBracketed(ref args) => {
                                    let arg = args.args.first().unwrap();
                                    match arg {
                                        syn::GenericArgument::Lifetime(_) => {}
                                        syn::GenericArgument::Type(ty) => {
                                            // parse the tuple of Read<T> and Write<T>
                                            let ty = &ty;
                                            match ty {
                                                syn::Type::Tuple(tup) => {
                                                    queries.push(quote! {
                                                        #tup
                                                    });
                                                }
                                                syn::Type::Path(path) => {
                                                    let ty_seg =
                                                        &path.path.segments.last().unwrap();
                                                    let inner = &ty_seg.arguments;
                                                    if ty_seg.ident == "Read"
                                                        || ty_seg.ident == "Write"
                                                    {
                                                        queries.push(quote! {
                                                            #path
                                                        });
                                                        match inner {
                                                            syn::PathArguments::AngleBracketed(
                                                                ref args,
                                                            ) => {
                                                                let arg = args.args.first().unwrap();
                                                                match arg {
                                                                    syn::GenericArgument::Type(
                                                                        ty,
                                                                    ) => {
                                                                        match ty {
                                                                            syn::Type::Path(path) => {
                                                                                if ty_seg.ident
                                                                                    == "Read"
                                                                                {
                                                                                    let ident = path
                                                                                        .path
                                                                                        .segments
                                                                                        .last()
                                                                                        .unwrap()
                                                                                        .ident
                                                                                        .to_string().to_owned();
                                                                                    if reads.contains(&ident) {
                                                                                        panic!("Conflicting reads: {}", ident);
                                                                                    }
                                                                                    if writes.contains(&ident) {
                                                                                        panic!("Conflicting reads: {}", ident);
                                                                                    }
                                                                                    reads.push(
                                                                                        ident
                                                                                    );
                                                                                } else {
                                                                                    let ident = path
                                                                                        .path
                                                                                        .segments
                                                                                        .last()
                                                                                        .unwrap()
                                                                                        .ident.to_string().to_owned();
                                                                                    if reads.contains(&ident) {
                                                                                        panic!("Conflicting writes: {}", ident);
                                                                                    }
                                                                                    if writes.contains(&ident) {
                                                                                        panic!("Conflicting writes: {}", ident);
                                                                                    }
                                                                                    writes.push(
                                                                                        ident
                                                                                    );
                                                                                }
                                                                            }
                                                                            _ => panic!("Invalid argument type: expected path"),
                                                                        }
                                                                    }
                                                                    _ => panic!("Invalid argument type: expected type"),
                                                                }
                                                            }
                                                            _ => panic!("Invalid argument type: expected angle bracketed arguments"),
                                                        }
                                                    } else {
                                                        panic!("Invalid argument type: expected Read or Write")
                                                    }
                                                }
                                                _ => panic!(
                                                    "Invalid argument type: expected path or tuple"
                                                ),
                                            }
                                        }
                                        _ => panic!("Invalid argument type"),
                                    }
                                }
                                _ => panic!("Invalid argument type"),
                            }
                        }
                    }
                    _ => panic!("Invalid argument type"),
                }
            }
            _ => panic!("Invalid argument type"),
        }
    }

    let mut arg_names = Vec::new();
    let mut arg_decls = Vec::new();
    for arg in args.iter() {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                let pat = &pat_type.pat;
                match &**pat {
                    syn::Pat::Ident(ident) => {
                        let ident = &ident.ident;
                        arg_decls.push(quote! {
                            #ident
                        });
                        arg_names.push(quote! {
                            #ident
                        });
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
                    let mut #arg_decls = world.query::<#queries>();
                )*
                {
                    #body
                }
            }

            fn components_read(&self) -> Vec<u64> {
                let mut components = Vec::new();
                #(
                    components.extend_from_slice(&<#queries>::components_read());
                )*
                components
            }

            fn components_written(&self) -> Vec<u64> {
                let mut components = Vec::new();
                #(
                    components.extend_from_slice(&<#queries>::components_written());
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
        impl<'w, 'q, #(#query_names),*> Queryable<'w, 'q> for (#(#query_names),*)
        where
            'w: 'q,
            #(#query_names: Queryable<'w, 'q>),*
        {
            type Item = (#(#item_names),*);
            type ItemRef = (#(#item_refs),*);
            type Iter = Box<dyn Iterator<Item = Self::ItemRef> + 'q>;

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
