use lexer::LexedShader;
use loader::{
    LexedShaderCache, LoadedShader, LoadedShaderCache, TextureCache, TryEverythingTextureLoader,
};
use render::{ShaderBindGroupLayout, ShaderPipeline};
use weaver_app::{plugin::Plugin, App};
use weaver_asset::{AssetApp, Assets};
use weaver_core::texture::Texture;
use weaver_renderer::RenderApp;
use weaver_util::prelude::Result;

pub mod lexer;
pub mod loader;
pub mod parser;
pub mod render;

pub struct ShaderPlugin;

impl Plugin for ShaderPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.init_resource::<TextureCache>();
        app.init_resource::<LexedShaderCache>();
        app.init_resource::<LoadedShaderCache>();
        app.add_asset_loader::<Texture, TryEverythingTextureLoader>();
        app.add_asset::<LexedShader>();
        let mut shaders = Assets::<LoadedShader>::new();
        shaders.insert_manual(
            loader::make_error_shader("textures/error"),
            loader::ERROR_SHADER_HANDLE.id(),
        );
        app.insert_resource(shaders);

        Ok(())
    }

    fn finish(&self, app: &mut App) -> Result<()> {
        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.init_resource::<ShaderBindGroupLayout>();
        render_app.init_resource::<ShaderPipeline>();
        Ok(())
    }
}
