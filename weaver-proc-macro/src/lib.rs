use quote::{format_ident, quote};


#[proc_macro_derive(Component)]
pub fn component_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_component_macro(&ast)
}

fn impl_component_macro(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;

    let gen = quote! {
        #[cfg_attr(feature = "serde", typetag::serde)]
        impl weaver_ecs::component::Component for #name {}
    };
    gen.into()
}

#[proc_macro_derive(Resource)]
pub fn resource_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_resource_macro(&ast)
}

fn impl_resource_macro(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;

    let gen = quote! {
        impl weaver_ecs::resource::Resource for #name {}
    };
    gen.into()
}

enum GpuComponentMemberType {
    Handle,
    HandleVec,
    HandleMap,
    Component,
    ComponentVec,
    ComponentMap,
    ComponentOption,
}

struct GpuComponentMember {
    name: syn::Ident,
    ty: GpuComponentMemberType,
}

#[proc_macro_derive(
    GpuComponent,
    attributes(gpu)
)]
pub fn gpu_component_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_gpu_component_macro(&ast)
}

fn impl_gpu_component_macro(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;

    let mut gpu_update = None;
    let mut members = Vec::new();

    let fields = match &ast.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => &fields.named,
            _ => panic!("Invalid struct"),
        },
        _ => panic!("Invalid struct"),
    };

    for attr in &ast.attrs {
        if let syn::Meta::List(list) = &attr.meta {
            let meta_ident = list.path.segments.last().unwrap().ident.to_string();
            if let "gpu" = meta_ident.as_str() {
                list.parse_args_with(|input: &syn::parse::ParseBuffer<'_>| {
                    while !input.is_empty() {
                        let ident = input.parse::<syn::Ident>().unwrap();
                        match ident.to_string().as_str() {
                            "update" => {
                                input.parse::<syn::Token![=]>().unwrap();
                                let ident = input.parse::<syn::LitStr>().unwrap();
                                gpu_update = Some(format_ident!("{}", ident.value()));
                            }
                            _ => panic!("Invalid attribute"),
                        }
                        if !input.is_empty() {
                            input.parse::<syn::Token![,]>().unwrap();
                        }
                    }

                    Ok(())
                }).unwrap();
            }
        }
    }

    for field in fields.iter() {
        let name = field.ident.clone().unwrap();
        let attrs = &field.attrs;

        let is_handle;
        let ty = &field.ty;
        let ident = match ty {
            syn::Type::Path(path) => &path.path.segments.last().unwrap().ident,
            _ => continue,
        };
        match ident.to_string().as_str() {
            "LazyGpuHandle" => {
                members.push(GpuComponentMember {
                    name: name.clone(),
                    ty: GpuComponentMemberType::Handle,
                });
                is_handle = true;
            }
            "Vec" => match ty {
                syn::Type::Path(path) => {
                    let path = &path.path;
                    let path_ident =
                        path.segments.last().unwrap().ident.to_string();
                    match path_ident.as_str() {
                        "LazyGpuHandle" => {
                            members.push(GpuComponentMember {
                                name: name.clone(),
                                ty: GpuComponentMemberType::HandleVec,
                            });
                            is_handle = true;
                        }
                        _ => {
                            is_handle = false;
                        }
                    }
                }
                _ => {
                    is_handle = false;
                }
            },
            "HashMap" | "FxHashMap" => match ty {
                syn::Type::Path(path) => {
                    let path = &path.path;
                    let path_ident =
                        path.segments.last().unwrap().ident.to_string();
                    match path_ident.as_str() {
                        "LazyGpuHandle" => {
                            members.push(GpuComponentMember {
                                name: name.clone(),
                                ty: GpuComponentMemberType::HandleMap,
                            });
                            is_handle = true;
                        }
                        _ => {
                            is_handle = false;
                        }
                    }
                }
                _ => {
                    is_handle = false;
                }
            },
            _ => {
                is_handle = false;
            }
        }

        for attr in attrs.iter() {
            let meta = &attr.meta;
            if let syn::Meta::List(list) = meta {
                let path_ident = list.path.segments.last().unwrap().ident.to_string();
                if let "gpu" = path_ident.as_str() {
                    if !is_handle {
                        list.parse_args_with(|input: &syn::parse::ParseBuffer<'_>| {
                            while !input.is_empty() {
                                let ident = input.parse::<syn::Ident>().unwrap();
                                if let "component" = ident.to_string().as_str() {
                                    let ty = &field.ty;
                                    let ident = match ty {
                                        syn::Type::Path(path) => &path.path.segments.last().unwrap().ident,
                                        _ => panic!("Invalid attribute"),
                                    };
                                    match ident.to_string().as_str() {
                                        "Vec" => {
                                            members.push(GpuComponentMember {
                                                name: name.clone(),
                                                ty: GpuComponentMemberType::ComponentVec,
                                            });
                                        }
                                        "HashMap" | "FxHashMap" => {
                                            members.push(GpuComponentMember {
                                                name: name.clone(),
                                                ty: GpuComponentMemberType::ComponentMap,
                                            });
                                        }
                                        "Option" => {
                                            members.push(GpuComponentMember {
                                                name: name.clone(),
                                                ty: GpuComponentMemberType::ComponentOption,
                                            });
                                        }
                                        _ => {
                                            members.push(GpuComponentMember {
                                                name: name.clone(),
                                                ty: GpuComponentMemberType::Component,
                                            });
                                        }
                                    }
                                }
                                if !input.is_empty() {
                                    input.parse::<syn::Token![,]>().unwrap();
                                }
                            }

                            Ok(())
                        }).unwrap();
                    }
                }
            }
        }
    }

    let mut lazy_init = Vec::new();
    let mut update_resources = Vec::new();
    let mut destroy_resources = Vec::new();

    for member in members {
        let name = &member.name;
        let ty = &member.ty;
        match ty {
            GpuComponentMemberType::Handle => {
                lazy_init.push(quote! {
                    self.#name.lazy_init(manager)?;
                });
                destroy_resources.push(quote! {
                    self.#name.mark_destroyed();
                });
            }
            GpuComponentMemberType::HandleVec => {
                lazy_init.push(quote! {
                    for handle in self.#name.iter() {
                        handle.lazy_init(manager)?;
                    }
                });
                destroy_resources.push(quote! {
                    for handle in self.#name.iter() {
                        handle.mark_destroyed();
                    }
                });
            }
            GpuComponentMemberType::HandleMap => {
                lazy_init.push(quote! {
                    for handle in self.#name.values() {
                        handle.lazy_init(manager)?;
                    }
                });
                destroy_resources.push(quote! {
                    for handle in self.#name.values() {
                        handle.mark_destroyed();
                    }
                });
            }
            GpuComponentMemberType::Component => {
                lazy_init.push(quote! {
                    self.#name.lazy_init(manager)?;
                });
                update_resources.push(quote! {
                    self.#name.update_resources(world)?;
                });
                destroy_resources.push(quote! {
                    self.#name.destroy_resources()?;
                });
            }
            GpuComponentMemberType::ComponentVec => {
                lazy_init.push(quote! {
                    for component in self.#name.iter() {
                        component.lazy_init(manager)?;
                    }
                });
                update_resources.push(quote! {
                    for component in self.#name.iter() {
                        component.update_resources(world)?;
                    }
                });
                destroy_resources.push(quote! {
                    for component in self.#name.iter() {
                        component.destroy_resources()?;
                    }
                });
            }
            GpuComponentMemberType::ComponentMap => {
                lazy_init.push(quote! {
                    for component in self.#name.values() {
                        component.lazy_init(manager)?;
                    }
                });
                update_resources.push(quote! {
                    for component in self.#name.values() {
                        component.update_resources(world)?;
                    }
                });
                destroy_resources.push(quote! {
                    for component in self.#name.values() {
                        component.destroy_resources()?;
                    }
                });
            }
            GpuComponentMemberType::ComponentOption => {
                lazy_init.push(quote! {
                    if let Some(component) = &self.#name {
                        component.lazy_init(manager)?;
                    }
                });
                update_resources.push(quote! {
                    if let Some(component) = &self.#name {
                        component.update_resources(world)?;
                    }
                });
                destroy_resources.push(quote! {
                    if let Some(component) = &self.#name {
                        component.destroy_resources()?;
                    }
                });
            }
        }
    }

    let gen = quote! {
        impl crate::renderer::internals::GpuComponent for #name {
            fn lazy_init(&self, manager: &crate::renderer::internals::GpuResourceManager) -> anyhow::Result<()> {
                #(
                    #lazy_init
                )*
                Ok(())
            }

            fn update_resources(&self, world: &weaver_ecs::World) -> anyhow::Result<()> {
                self.#gpu_update(world)?;
                #(
                    #update_resources
                )*
                Ok(())
            }

            fn destroy_resources(&self) -> anyhow::Result<()> {
                #(
                    #destroy_resources
                )*
                Ok(())
            }
        }
    };
    gen.into()
}

enum BindingType {
    Uniform,
    Storage {
        read_only: bool,
    },
    Texture {
        format: proc_macro2::TokenStream,
        sample_type: proc_macro2::TokenStream,
        view_dimension: proc_macro2::TokenStream,
        layers: proc_macro2::TokenStream,
    },
    Sampler {
        filtering: bool,
        comparison: bool,
    },
}

struct Binding {
    name: syn::Ident,
    binding_type: BindingType,
    default: Option<syn::Path>,
}

#[proc_macro_derive(BindableComponent, attributes(uniform, texture, sampler, storage))]
pub fn bindable_component_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_bindable_component_macro(&ast)
}

fn impl_bindable_component_macro(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;

    let mut bindings = Vec::new();

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
        let ty = &field.ty;
        let ty_ident = match ty {
            syn::Type::Path(path) => &path.path.segments.last().unwrap().ident,
            _ => panic!("Invalid attribute"),
        };
        let is_optional = ty_ident.to_string().as_str() == "Option";
        

        for attr in attrs.iter() {
            let attr = &attr.meta;
            let mut default = None;
            match attr {
                syn::Meta::Path(path) => {
                    let path_ident = path.segments.last().unwrap().ident.to_string();
                    
                    match path_ident.as_str() {
                        // #[uniform]
                        "uniform" => {
                            bindings.push(Binding {
                                name: name.clone(),
                                binding_type: BindingType::Uniform,
                                default: None,
                            });
                        }
                        // #[storage]
                        "storage" => {
                            bindings.push(Binding {
                                name: name.clone(),
                                binding_type: BindingType::Storage {
                                    read_only: true,
                                },
                                default: None,
                            });
                        }
                        // #[sampler]
                        "sampler" => {
                            bindings.push(Binding {
                                name: name.clone(),
                                binding_type: BindingType::Sampler {
                                    filtering: false,
                                    comparison: false,
                                },
                                default: None,
                            });
                        }
                        _ => {}
                    }
                }
                syn::Meta::List(list) => {
                    let path_ident = list.path.segments.last().unwrap().ident.to_string();
                    
                    match path_ident.as_str() {
                        // #[texture(...)]
                        "texture" => {
                            let mut sample_type = None;
                            let mut view_dimension = None;
                            let mut layers = None;
                            let mut format = None;
                            list.parse_args_with(|input: syn::parse::ParseStream| {
                                while !input.is_empty() {
                                    let ident = input.parse::<syn::Ident>().unwrap();
                                    match ident.to_string().as_str() {
                                        // #[texture(sample_type = ...)]
                                        "sample_type" => {
                                            input.parse::<syn::Token![=]>().unwrap();
                                            let ident =
                                                input.parse::<syn::Ident>().unwrap().to_string();
                                            match ident.as_str() {
                                                "filterable_float" => {
                                                    sample_type = Some(quote! { wgpu::TextureSampleType::Float { filterable: true } })
                                                }
                                                "float" => {
                                                    sample_type = Some(quote! { wgpu::TextureSampleType::Float { filterable: false } })
                                                }
                                                "depth" => {
                                                    sample_type = Some(quote! { wgpu::TextureSampleType::Depth })
                                                }
                                                _ => panic!("Invalid attribute"),
                                            }
                                        }
                                        // #[texture(view_dimension = ...)]
                                        "view_dimension" => {
                                            input.parse::<syn::Token![=]>().unwrap();
                                            let ident =
                                                input.parse::<syn::Variant>().unwrap();
                                            view_dimension = Some(quote! { wgpu::TextureViewDimension::#ident })
                                            
                                        }
                                        // #[texture(layers = ...)]
                                        "layers" => {
                                            input.parse::<syn::Token![=]>().unwrap();
                                            let lit =
                                                input.parse::<syn::LitInt>().unwrap();
                                            let lit = lit.base10_parse::<u32>().unwrap();
                                            layers = Some(quote! { Some(#lit) })
                                        }
                                        // #[texture(format = ...)]
                                        "format" => {
                                            input.parse::<syn::Token![=]>().unwrap();
                                            let ident =
                                                input.parse::<syn::Variant>().unwrap();
                                            format = Some(quote! { 
                                                wgpu::TextureFormat::#ident
                                            });
                                        }
                                        "default"  => {
                                            if is_optional {
                                                input.parse::<syn::Token![=]>().unwrap();
                                                if default.is_none() {
                                                    default = Some(input.parse::<syn::Path>().unwrap());
                                                }
                                            } else {
                                                panic!("Expected a `default` attribute on Option fields");
                                            }
                                        }
                                        _ => panic!("Invalid attribute"),
                                    }
                                    if !input.is_empty() {
                                        input.parse::<syn::Token![,]>().unwrap();
                                    }
                                }

                                Ok(())
                            })
                            .unwrap();

                            if is_optional && default.is_none() {
                                panic!("Expected a `default` attribute on Option fields");
                            }

                            bindings.push(Binding {
                                name: name.clone(),
                                binding_type: BindingType::Texture {
                                    format: format.expect("Missing format attribute"),
                                    sample_type: sample_type.expect("Missing sample_type attribute"),
                                    view_dimension: view_dimension.expect("Missing view_dimension attribute"),
                                    layers: layers.unwrap_or(quote! { None }),
                                },
                                default,
                            });
                        }

                        // #[sampler(...)]
                        "sampler" => {
                            let mut filtering = None;
                            let mut comparison = None;
                            list.parse_args_with(|input: syn::parse::ParseStream| {
                                while !input.is_empty() {
                                    let ident = input.parse::<syn::Ident>().unwrap();
                                    match ident.to_string().as_str() {
                                        // #[sampler(filtering = ...)]
                                        "filtering" => {
                                            input.parse::<syn::Token![=]>().unwrap();
                                            let ident =
                                                input.parse::<syn::LitBool>().unwrap();
                                            filtering = Some(ident.value);
                                        }
                                        // #[sampler(comparison = ...)]
                                        "comparison" => {
                                            input.parse::<syn::Token![=]>().unwrap();
                                            let ident =
                                                input.parse::<syn::LitBool>().unwrap();
                                            comparison = Some(ident.value);
                                        }
                                        "default"  => {
                                            if is_optional {
                                                input.parse::<syn::Token![=]>().unwrap();
                                                default = Some(input.parse::<syn::Path>().unwrap());
                                            } else {
                                                panic!("Expected a `default` attribute on Option fields");
                                            }
                                        }
                                        _ => panic!("Invalid attribute"),
                                    }
                                    if !input.is_empty() {
                                        input.parse::<syn::Token![,]>().unwrap();
                                    }
                                }

                                Ok(())
                            })
                            .unwrap();
                            
                            bindings.push(Binding {
                                name: name.clone(),
                                binding_type: BindingType::Sampler {
                                    filtering: filtering.unwrap_or(true),
                                    comparison: comparison.unwrap_or(false),
                                },
                                default,
                            });
                        }

                        // #[storage(...)]
                        "storage" => {
                            let mut read_write = None;
                            list.parse_args_with(|input: syn::parse::ParseStream| {
                                while !input.is_empty() {
                                    let ident = input.parse::<syn::Ident>().unwrap();
                                    match ident.to_string().as_str() {
                                        // #[storage(read_write = ...)]
                                        "read_write" => {
                                            input.parse::<syn::Token![=]>().unwrap();
                                            let ident =
                                                input.parse::<syn::LitBool>().unwrap();
                                            read_write = Some(ident.value);
                                        }
                                        "default"  => {
                                            if is_optional {
                                                input.parse::<syn::Token![=]>().unwrap();
                                                default = Some(input.parse::<syn::Path>().unwrap());
                                            } else {
                                                panic!("Expected a `default` attribute on Option fields");
                                            }
                                        }
                                        _ => panic!("Invalid attribute"),
                                    }
                                    if !input.is_empty() {
                                        input.parse::<syn::Token![,]>().unwrap();
                                    }
                                }

                                Ok(())
                            })
                            .unwrap();
                            
                            bindings.push(Binding {
                                name: name.clone(),
                                binding_type: BindingType::Storage {
                                    read_only: !read_write.unwrap_or(false),
                                },
                                default,
                            });
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    let mut binding_layout_entries = Vec::new();
    let mut binding_entries = Vec::new();
    let mut binding_creations = Vec::new();

    for (i, binding) in bindings.iter().enumerate() {
        let binding_index = i as u32;
        let name = &binding.name;
        let visibility = match &binding.binding_type {
            BindingType::Uniform => {
                quote! { wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT }
            }
            BindingType::Storage { .. } => {
                quote! { wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT }
            }
            BindingType::Texture { .. } => {
                quote! { wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE }
            }
            BindingType::Sampler { .. } => {
                quote! { wgpu::ShaderStages::FRAGMENT }
            }
        };
        let ty = match &binding.binding_type {
            BindingType::Uniform => {
                quote! { wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None } }
            }
            BindingType::Storage { read_only } => {
                quote! { wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: #read_only }, has_dynamic_offset: false, min_binding_size: None } }
            }
            BindingType::Texture { sample_type, view_dimension, .. } => {
                quote! { wgpu::BindingType::Texture { 
                    sample_type: #sample_type, 
                    view_dimension: #view_dimension, 
                    multisampled: false, 
                 } }
            }
            BindingType::Sampler {
                filtering,
                comparison,
                ..
            } => {
                if *comparison {
                    quote! { wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison) }
                } else if *filtering {
                    quote! { wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering) }
                } else {
                    quote! { wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering) }
                }
            }
        };
        binding_layout_entries.push(quote! {
            wgpu::BindGroupLayoutEntry {
                binding: #binding_index,
                visibility: #visibility,
                ty: #ty,
                count: None,
            }
        });

        let binding_entry = match &binding.binding_type {
            BindingType::Uniform => {
                quote! { wgpu::BindingResource::Buffer(#name.as_entire_buffer_binding()) }
            }
            BindingType::Storage { .. } => {
                quote! { wgpu::BindingResource::Buffer(#name.as_entire_buffer_binding()) }
            }
            BindingType::Texture { .. } => {
                quote! { wgpu::BindingResource::TextureView(&#name) }
            }
            BindingType::Sampler { .. } => {
                quote! { wgpu::BindingResource::Sampler(&#name) }
            }
        };

        let mut binding_creation;

        if let Some(default) = &binding.default {
            binding_creation = quote! { let #name = if let Some(#name) = self.#name.clone() { #name } else { #default() }; };
        } else {
            binding_creation = quote! { let #name = &self.#name; };
        }

        binding_creation.extend(match &binding.binding_type {
            BindingType::Uniform => {
                quote! {
                    let #name = #name.lazy_init(manager)?;
                    let #name = #name.get_buffer().unwrap();
                }
            }
            BindingType::Storage { .. } => {
                quote! {
                    let #name = #name.lazy_init(manager)?;
                    let #name = #name.get_buffer().unwrap();
                }
            }
            BindingType::Texture { format, view_dimension, layers, .. } => {
                quote! {
                    let #name = #name.handle().lazy_init(manager)?;
                    let #name = #name.get_texture().unwrap();
                    let #name = #name.create_view(&wgpu::TextureViewDescriptor {
                        label: Some(concat!(stringify!(#name), " Texture View")),
                        format: Some(#format),
                        dimension: Some(#view_dimension),
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: 0,
                        mip_level_count: None,
                        base_array_layer: 0,
                        array_layer_count: #layers,
                    });
                }
            }
            BindingType::Sampler { .. } => {
                quote! {
                    let #name = #name.lazy_init(manager)?;
                    let #name = #name.get_sampler().unwrap();
                }
            }
        });

        binding_entries.push(quote! {
            wgpu::BindGroupEntry {
                binding: #binding_index,
                resource: #binding_entry,
            }
        });
        binding_creations.push(binding_creation);
    }

    let gen = quote! {
        impl crate::renderer::internals::BindableComponent for #name {
            fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some(concat!(stringify!(#name), " Bind Group Layout")),
                    entries: &[#(#binding_layout_entries),*],
                })
            }

            fn create_bind_group(
                &self,
                manager: &crate::renderer::internals::GpuResourceManager,
                cache: &crate::renderer::internals::BindGroupLayoutCache,
            ) -> anyhow::Result<std::sync::Arc<wgpu::BindGroup>> {
                let layout = cache.get_or_create::<Self>(manager.device());
                #(
                    #binding_creations
                )*
                let bind_group = manager
                    .device()
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some(concat!(stringify!(#name), " Bind Group")),
                        layout: &layout,
                        entries: &[#(#binding_entries),*],
                    });

                Ok(std::sync::Arc::new(bind_group))
            }

            fn bind_group(&self) -> Option<Arc<wgpu::BindGroup>> {
                self.bind_group.bind_group().clone()
            }

            fn lazy_init_bind_group(
                &self,
                manager: &crate::renderer::internals::GpuResourceManager,
                cache: &crate::renderer::internals::BindGroupLayoutCache,
            ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
                if let Some(bind_group) = self.bind_group.bind_group() {
                    return Ok(bind_group);
                }

                let bind_group = self.bind_group.lazy_init_bind_group(manager, cache, self)?;
                Ok(bind_group)
            }
        }
    };

    gen.into()
}

#[proc_macro_attribute]
pub fn system(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let ast = syn::parse(item).unwrap();
    impl_system_macro(attr, &ast)
}

fn impl_system_macro(attr: proc_macro::TokenStream, ast: &syn::ItemFn) -> proc_macro::TokenStream {
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
        Some(ref commands) => quote! { let mut #commands = weaver_ecs::commands::Commands::new(&world.read()); },
        None => quote! {},
    };

    let commands_finalize = match commands_binding {
        Some(commands) => quote! { {
            #commands.finalize(&mut world.write());
        } },
        None => quote! {},
    };

    let run_fn = quote! {
        fn run(&self, world: std::sync::Arc<parking_lot::RwLock<weaver_ecs::world::World>>) -> anyhow::Result<()> {
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

            fn components_read(&self) -> Vec<weaver_ecs::StaticId> {
                use weaver_ecs::query::Queryable;
                let mut components = Vec::new();
                #(
                    components.extend(<#query_types as Queryable<#filter_types>>::access().reads.ones().map(|id| id as weaver_ecs::StaticId));
                )*
                components
            }

            fn components_written(&self) -> Vec<weaver_ecs::StaticId> {
                use weaver_ecs::query::Queryable;
                let mut components = Vec::new();
                #(
                    components.extend(<#query_types as Queryable<#filter_types>>::access().writes.ones().map(|id| id as weaver_ecs::StaticId));
                )*
                components
            }

            fn resources_read(&self) -> Vec<weaver_ecs::StaticId> {
                let mut resources = Vec::new();
                #(
                    resources.push(weaver_ecs::static_id::<#res_types>());
                )*
                resources
            }

            fn resources_written(&self) -> Vec<weaver_ecs::StaticId> {
                let mut resources = Vec::new();
                #(
                    resources.push(weaver_ecs::static_id::<#resmut_types>());
                )*
                resources
            }

            fn is_exclusive(&self) -> bool {
                false // todo
            }
        }
    };
    gen.into()
}

#[proc_macro_derive(Bundle)]
pub fn bundle_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_bundle_macro(&ast)
}

fn impl_bundle_macro(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
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
            fn build_on(self, entity: weaver_ecs::entity::Entity, world: &mut weaver_ecs::storage::Components) -> weaver_ecs::entity::Entity {
                #(
                    self.#fields.build_on(entity, world);
                )*
                entity
            }
        }
    };
    gen.into()
}

#[proc_macro]
pub fn impl_queryable_for_n_tuple(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
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
        impl<'a, #(#names),*, F> crate::query::Queryable<'a, F> for (#(#names),*)
        where
            F: crate::query::QueryFilter<'a>,
            #(#names: crate::query::Queryable<'a, F>,)*
            #(#names::Item: crate::Component,)*
        {
            type Item = (#(#names::Item),*);
            type ItemRef = (#(#names::ItemRef),*);

            fn get(entries: &'a ComponentMap) -> Option<Self::ItemRef> {
                #(
                    let #names = #names::get(entries)?;
                )*
                Some((#(#names),*))
            }

            fn reads() -> Option<ComponentSet> {
                let mut reads = ComponentSet::default();
                #(
                    reads.extend(#names::reads().unwrap_or_default());
                )*
                Some(reads)
            }

            fn writes() -> Option<ComponentSet> {
                let mut writes = ComponentSet::default();
                #(
                    writes.extend(#names::writes().unwrap_or_default());
                )*
                Some(writes)
            }

            fn withs() -> Option<ComponentSet> {
                let mut withs = ComponentSet::default();
                #(
                    withs.extend(#names::withs().unwrap_or_default());
                )*
                Some(withs)
            }

            fn withouts() -> Option<ComponentSet> {
                let mut withouts = ComponentSet::default();
                #(
                    withouts.extend(#names::withouts().unwrap_or_default());
                )*
                Some(withouts)
            }
        }
    };

    gen.into()
}
