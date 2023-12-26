use proc_macro::*;
use quote::quote;

static COMPONENT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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
    let gen = quote! {
        impl weaver_ecs::resource::Resource for #name {}
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

    // assert that all arguments are `Read`
    for arg in args {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                let ty = &pat_type.ty;
                match &**ty {
                    syn::Type::Path(path) => {
                        let segments = &path.path.segments;
                        let segment = segments.last().unwrap();
                        let ident = &segment.ident;
                        if ident != "Read" {
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

    let arg_names = args
        .iter()
        .map(|arg| match arg {
            syn::FnArg::Typed(pat_type) => &pat_type.pat,
            _ => panic!("Invalid argument type"),
        })
        .collect::<Vec<_>>();

    let gen = quote! {
        #ast

        #[allow(non_camel_case_types, dead_code)]
        #vis struct #system_struct_name;

        impl weaver_ecs::system::System for #system_struct_name {
            fn run(&self, world: &weaver_ecs::World) {
                #(
                    let #arg_names = world.query::<#arg_types>();
                )*;
                #name(#(#arg_names),*);
            }
        }
    };
    gen.into()
}
