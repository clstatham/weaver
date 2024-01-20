use quote::quote;

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

pub fn derive_bindable_component(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
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

            fn bind_group(&self) -> Option<std::sync::Arc<wgpu::BindGroup>> {
                self.bind_group.bind_group().clone()
            }

            fn lazy_init_bind_group(
                &self,
                manager: &crate::renderer::internals::GpuResourceManager,
                cache: &crate::renderer::internals::BindGroupLayoutCache,
            ) -> anyhow::Result<std::sync::Arc<wgpu::BindGroup>> {
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