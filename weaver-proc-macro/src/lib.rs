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
    let generics = &ast.generics;

    let generic_types = generics.type_params().map(|param| {
        let ident = &param.ident;
        quote! {
            #ident
        }
    });

    let id = COMPONENT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let gen = quote! {
        unsafe impl #generics crate::ecs::component::Component for #name<#(#generic_types),*> {
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
                    let mut #query_names: Query<#query_types, #filter_types> = Query::new(world);
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
                use crate::ecs::query::Queryable;
                let mut components = Vec::new();
                #(
                    components.extend_from_slice(&<#query_types as Queryable<#filter_types>>::reads().unwrap_or_default().into_iter().collect::<Vec<_>>());
                )*
                components
            }

            fn components_written(&self) -> Vec<u64> {
                use crate::ecs::query::Queryable;
                let mut components = Vec::new();
                #(
                    components.extend_from_slice(&<#query_types as Queryable<#filter_types>>::writes().unwrap_or_default().into_iter().collect::<Vec<_>>());
                )*
                components
            }

            fn resources_read(&self) -> Vec<u64> {
                let mut resources = Vec::new();
                #(
                    resources.push(<#res_types as crate::ecs::resource::Resource>::resource_id());
                )*
                resources
            }

            fn resources_written(&self) -> Vec<u64> {
                let mut resources = Vec::new();
                #(
                    resources.push(<#resmut_types as crate::ecs::resource::Resource>::resource_id());
                )*
                resources
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
            fn build_on(self, entity: crate::ecs::entity::Entity, world: &crate::ecs::world::World) -> anyhow::Result<crate::ecs::entity::Entity> {
                #(
                    self.#fields.build_on(entity, world)?;
                )*
                Ok(entity)
            }
        }
    };
    gen.into()
}

#[proc_macro]
pub fn impl_queryable_for_n_tuple(input: TokenStream) -> TokenStream {
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
        impl<'a, #(#names),*, F> crate::ecs::query::Queryable<'a, F> for (#(#names),*)
        where
            F: crate::ecs::query::QueryFilter<'a>,
            #(#names: crate::ecs::query::Queryable<'a, F>,)*
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

            fn withs() -> Option<crate::ecs::query::FxHashSet<u64>> {
                let mut withs = crate::ecs::query::FxHashSet::default();
                #(
                    withs.extend(&#names::withs().unwrap_or_default().into_iter().collect::<Vec<_>>());
                )*
                Some(withs)
            }

            fn withouts() -> Option<crate::ecs::query::FxHashSet<u64>> {
                let mut withouts = crate::ecs::query::FxHashSet::default();
                #(
                    withouts.extend(&#names::withouts().unwrap_or_default().into_iter().collect::<Vec<_>>());
                )*
                Some(withouts)
            }

            fn ors() -> Option<crate::ecs::query::FxHashSet<(u64, u64)>> {
                let mut ors = crate::ecs::query::FxHashSet::default();
                #(
                    ors.extend(&#names::ors().unwrap_or_default().into_iter().collect::<Vec<_>>());
                )*
                Some(ors)
            }

            fn maybes() -> Option<crate::ecs::query::FxHashSet<u64>> {
                let mut maybes = crate::ecs::query::FxHashSet::default();
                #(
                    maybes.extend(&#names::maybes().unwrap_or_default().into_iter().collect::<Vec<_>>());
                )*
                Some(maybes)
            }
        }
    };

    gen.into()
}

enum BindingType {
    Uniform,
    Storage { read_only: bool },
}

#[proc_macro_derive(RenderResource)]
pub fn render_resource_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_render_resource_macro(&ast)
}

fn impl_render_resource_macro(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;

    // Automatically create a bind group layout for the resource
    // anything marked with the attr `#[uniform]` will be included in the bind group layout and will be a uniform buffer
    // anything marked with the attr `#[texture]` will be included in the bind group layout and will be a texture
    // anything marked with the attr `#[storage_read]` will be included in the bind group layout and will be a storage buffer that is read only
    // anything marked with the attr `#[storage_read_write]` will be included in the bind group layout and will be a storage buffer that is read/write

    let mut binding_types = Vec::new();

    let fields = match &ast.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => &fields.named,
            _ => panic!("Invalid struct"),
        },
        _ => panic!("Invalid struct"),
    };

    for field in fields.iter() {
        let name = field.ident.clone().unwrap();
        let attrs = &field.attrs;

        for attr in attrs.iter() {
            let attr = &attr.meta;
            match attr {
                syn::Meta::Path(path) => {
                    let path_ident = path.segments.last().unwrap().ident.to_string();
                    match path_ident.as_str() {
                        "uniform" => {
                            binding_types.push((name.clone(), BindingType::Uniform));
                        }
                        "texture" => {
                            todo!();
                        }
                        "storage_read" => {
                            binding_types
                                .push((name.clone(), BindingType::Storage { read_only: true }));
                        }
                        "storage_read_write" => {
                            binding_types
                                .push((name.clone(), BindingType::Storage { read_only: false }));
                        }
                        _ => panic!("Invalid attribute"),
                    }
                }
                _ => panic!("Invalid attribute"),
            }
        }
    }

    let mut binding_layout_entries = Vec::new();

    for (i, (_binding, binding_type)) in binding_types.iter().enumerate() {
        let binding_entry = match binding_type {
            BindingType::Uniform => quote! { wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            } },
            BindingType::Storage { read_only } => quote! { wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: #read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            } },
        };

        binding_layout_entries.push(quote! {
            wgpu::BindGroupLayoutEntry {
                binding: #i as u32,
                visibility: wgpu::ShaderStage::all(),
                ty: #binding_entry,
            }
        });
    }

    // generate the bind group
    let mut binding_group_entries = Vec::new();

    for (i, (binding, binding_type)) in binding_types.iter().enumerate() {
        let binding = format_ident!("{}", binding);
        let binding_entry = match binding_type {
            BindingType::Uniform => quote! { wgpu::Binding::Buffer {
                buffer: &self.#binding.as_entire_buffer_binding(),
                offset: 0,
                size: None,
            } },
            BindingType::Storage { .. } => quote! { wgpu::Binding::Buffer {
                buffer: &self.#binding.as_entire_buffer_binding(),
                offset: 0,
                size: None,
            } },
        };

        binding_group_entries.push(quote! {
            wgpu::BindGroupEntry {
                binding: #i as u32,
                resource: #binding_entry,
            }
        });
    }

    let gen = quote! {
        impl crate::renderer::RenderResource for #name {
            fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some(stringify!(#name)),
                    entries: &[
                        #(#binding_layout_entries),*
                    ],
                })
            }

            fn bind_group(&self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::BindGroup {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(stringify!(#name)),
                    layout,
                    entries: &[
                        #(#binding_group_entries),*
                    ],
                })
            }
        }
    };

    gen.into()
}
