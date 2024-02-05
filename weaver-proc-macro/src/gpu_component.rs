use quote::{format_ident, quote};

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

pub fn derive_gpu_component(ast: &syn::DeriveInput) -> proc_macro::TokenStream {
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
                })
                .unwrap();
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
                    let path_ident = path.segments.last().unwrap().ident.to_string();
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
                    let path_ident = path.segments.last().unwrap().ident.to_string();
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
                                        syn::Type::Path(path) => {
                                            &path.path.segments.last().unwrap().ident
                                        }
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
                        })
                        .unwrap();
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

            fn update_resources(&self, world: &fabricate::world::World) -> anyhow::Result<()> {
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
