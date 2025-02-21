use lexer::LexedShader;
use loader::{
    LexedShaderCache, LoadedShader, LoadedShaderCache, TextureCache, TryEverythingTextureLoader,
};
use render::{ShaderBindGroupLayout, ShaderPipelineCache};
use weaver_app::{App, plugin::Plugin};
use weaver_asset::{AssetApp, Assets};
use weaver_renderer::{RenderApp, WgpuDevice};
use weaver_util::prelude::*;

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
        app.add_asset_loader::<TryEverythingTextureLoader, _>();
        app.add_asset::<LexedShader>();
        let mut shaders = Assets::<LoadedShader>::new();
        shaders.insert_manual(
            loader::make_error_shader("textures/error"),
            loader::ERROR_SHADER_HANDLE.id(),
        );
        app.insert_resource(shaders);

        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.add_plugin(ShaderRenderAppPlugin).unwrap();

        Ok(())
    }
}

pub struct ShaderRenderAppPlugin;

impl Plugin for ShaderRenderAppPlugin {
    fn build(&self, _app: &mut App) -> Result<()> {
        Ok(())
    }

    fn ready(&self, app: &App) -> bool {
        app.has_resource::<WgpuDevice>()
    }

    fn finish(&self, render_app: &mut App) -> Result<()> {
        render_app.init_resource::<ShaderBindGroupLayout>();
        render_app.init_resource::<ShaderPipelineCache>();

        Ok(())
    }
}
