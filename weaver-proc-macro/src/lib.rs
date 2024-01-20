use proc_macro::TokenStream;

mod bindable_component;
mod bundle;
mod component;
mod gpu_component;
mod resource;
mod system;

#[proc_macro_derive(Component)]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as syn::DeriveInput);
    component::derive_component(&ast)
}

#[proc_macro_derive(Bundle)]
pub fn derive_bundle(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as syn::DeriveInput);
    bundle::derive_bundle(&ast)
}

#[proc_macro_derive(Resource)]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as syn::DeriveInput);
    resource::derive_resource(&ast)
}

#[proc_macro_derive(GpuComponent, attributes(gpu))]
pub fn derive_gpu_component(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as syn::DeriveInput);
    gpu_component::derive_gpu_component(&ast)
}

#[proc_macro_derive(BindableComponent, attributes(uniform, texture, sampler, storage))]
pub fn derive_bindable_component(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as syn::DeriveInput);
    bindable_component::derive_bindable_component(&ast)
}

#[proc_macro_attribute]
pub fn system(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(item as syn::ItemFn);
    system::system(attr, &ast)
}
