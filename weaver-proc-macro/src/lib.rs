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
    let name = &ast.sig.ident;
    // let system_struct_name = format_ident!("{}System", name);
    // get the name from the first attr
    let system_struct_name = syn::parse::<syn::Ident>(attr).unwrap();
    let args = &ast.sig.inputs;

    let mut arg_types = Vec::new();
    let mut arg_read_or_write = Vec::new();

    // assert that all arguments are `Read` or `Write`
    for arg in args {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                let ty = &pat_type.ty;
                match &**ty {
                    syn::Type::Path(path) => {
                        let segments = &path.path.segments;
                        let segment = segments.last().unwrap();
                        let ident = &segment.ident;
                        if ident == "Read" {
                            arg_read_or_write.push(format_ident!("read"));
                        } else if ident == "Write" {
                            arg_read_or_write.push(format_ident!("write"));
                        } else {
                            panic!("Invalid argument type");
                        }

                        let generic_args = &segment.arguments;
                        match generic_args {
                            syn::PathArguments::AngleBracketed(args) => {
                                let arg = args.args.first().unwrap();
                                match arg {
                                    syn::GenericArgument::Type(ty) => {
                                        arg_types.push(ty);
                                    }
                                    syn::GenericArgument::Lifetime(_) => {
                                        let arg = args.args.iter().nth(1).unwrap();
                                        match arg {
                                            syn::GenericArgument::Type(ty) => {
                                                arg_types.push(ty);
                                            }
                                            _ => panic!("Invalid argument type"),
                                        }
                                    }
                                    _ => panic!("Invalid argument type"),
                                }
                            }
                            _ => panic!("Invalid argument type"),
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
                        let muta = &ident.mutability;
                        let ident = &ident.ident;
                        arg_decls.push(quote! {
                            #muta #ident
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

    let gen = quote! {
        #ast

        #[allow(non_camel_case_types, dead_code)]
        #vis struct #system_struct_name;

        impl weaver_ecs::system::System for #system_struct_name {
            #[allow(unused_mut)]
            fn run(&self, world: &weaver_ecs::World) {
                #(
                    let #arg_decls = world.#arg_read_or_write::<#arg_types>();
                )*
                #name(#(#arg_names),*);
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
