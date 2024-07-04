use lexer::LexedShader;
use loader::{LoadedShader, ShaderCache, TextureCache, TryEverythingTextureLoader};
use render::{
    extract::ExtractedShader, init_keyed_shader_stage_pipelines, KeyedShaderStage,
    KeyedShaderStagePipelineCache,
};
use weaver_app::{plugin::Plugin, App};
use weaver_asset::{AddAsset, Assets};
use weaver_core::texture::Texture;
use weaver_renderer::{
    asset::ExtractRenderAssetPlugin, bind_group::BindGroup, InitRenderResources, RenderApp,
};
use weaver_util::prelude::Result;

pub mod lexer;
pub mod loader;
pub mod parser;
pub mod render;

pub struct ShaderPlugin;

impl Plugin for ShaderPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.init_resource::<TextureCache>();
        app.init_resource::<ShaderCache>();
        app.add_asset_loader::<Texture, TryEverythingTextureLoader>();
        app.add_asset::<LexedShader>();
        let mut shaders = Assets::<LoadedShader>::new();
        shaders.insert_manual(
            loader::make_error_shader("textures/error"),
            loader::ERROR_SHADER_HANDLE.id(),
        );
        app.insert_resource(shaders);

        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.add_plugin(ExtractRenderAssetPlugin::<ExtractedShader>::default())?;
        render_app.add_system(init_keyed_shader_stage_pipelines, InitRenderResources);
        render_app.init_resource::<KeyedShaderStagePipelineCache>();
        render_app.add_asset::<BindGroup<KeyedShaderStage>>();

        Ok(())
    }
}
