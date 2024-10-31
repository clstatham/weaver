use assets::material_mesh::{LoadedModelWithMaterials, ObjMaterialModelLoader};
use light::{PointLight, PointLightPlugin};
use material::{MaterialPlugin, BLACK_TEXTURE, ERROR_TEXTURE, WHITE_TEXTURE};
use prelude::Material;
use render::PbrLightingInformation;
use skybox::{render_skybox, Skybox, SkyboxPlugin, SkyboxRenderable, SkyboxRenderablePlugin};
use weaver_app::prelude::*;
use weaver_asset::{AssetApp, Assets};
use weaver_core::{texture::Texture, transform::Transform};
use weaver_ecs::{
    component::{Res, ResMut},
    prelude::Commands,
    query::Query,
};
use weaver_renderer::{
    bind_group::ResourceBindGroupPlugin,
    clear_color::{render_clear_color, ClearColorRenderable},
    hdr::{render_hdr, HdrRenderable},
    pipeline::RenderPipelinePlugin,
    InitRenderResources, Render, RenderApp, WgpuDevice, WgpuQueue,
};
use weaver_util::Result;

pub mod assets;
pub mod light;
pub mod material;
pub mod render;
pub mod skybox;

pub mod prelude {
    pub use crate::assets::material_mesh::*;
    pub use crate::light::*;
    pub use crate::material::*;
    pub use crate::skybox::*;
    pub use crate::PbrPlugin;
}

pub struct PbrPlugin;

impl Plugin for PbrPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_asset::<Material>();
        app.add_asset::<LoadedModelWithMaterials>();
        app.add_asset_loader::<LoadedModelWithMaterials, ObjMaterialModelLoader, _>();

        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.add_plugin(MaterialPlugin)?;
        render_app.add_plugin(PointLightPlugin)?;
        render_app.add_plugin(SkyboxPlugin)?;
        render_app.add_plugin(SkyboxRenderablePlugin)?;

        render_app.add_plugin(ResourceBindGroupPlugin::<PbrLightingInformation>::default())?;

        // render_app.add_plugin(RenderPipelinePlugin::<PbrRenderable>::default())?;

        // render_app.add_render_command::<_, PbrRenderCommand>();

        // render_app.add_renderable::<PbrRenderable>();
        // render_app.add_renderable_dependency::<PbrRenderable, ClearColorRenderable>();
        // render_app.add_renderable_dependency::<SkyboxRenderable, PbrRenderable>();

        render_app
            .world_mut()
            .add_system_dependency(render_skybox, render_clear_color, Render);
        render_app
            .world_mut()
            .add_system_after(render_hdr, render_skybox, Render);

        render_app
            .world_mut()
            .add_system(init_pbr_lighting_information, InitRenderResources);
        render_app.world_mut().add_system_after(
            update_pbr_lighting_information,
            init_pbr_lighting_information,
            InitRenderResources,
        );

        Ok(())
    }

    fn finish(&self, app: &mut App) -> Result<()> {
        let white_texture = Texture::from_rgba8(&[255, 255, 255, 255], 1, 1);
        let black_texture = Texture::from_rgba8(&[0, 0, 0, 255], 1, 1);
        let error_texture = Texture::from_rgba8(
            &[
                255, 0, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 0, 255, 255,
            ],
            2,
            2,
        );
        let mut textures = app
            .main_app_mut()
            .world_mut()
            .get_resource_mut::<Assets<Texture>>()
            .unwrap();
        textures.insert_manual(white_texture, WHITE_TEXTURE.id());
        textures.insert_manual(black_texture, BLACK_TEXTURE.id());
        textures.insert_manual(error_texture, ERROR_TEXTURE.id());
        Ok(())
    }
}

pub(crate) async fn init_pbr_lighting_information(mut commands: Commands, _skybox: Res<Skybox>) {
    if !commands.has_resource::<PbrLightingInformation>().await {
        commands.init_resource::<PbrLightingInformation>().await;
    }
}

pub(crate) async fn update_pbr_lighting_information(
    mut lighting: ResMut<PbrLightingInformation>,
    mut lights: Query<(&PointLight, Option<&Transform>)>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) {
    lighting.point_lights.buffer.clear();
    for (light, transform) in lights.iter() {
        if let Some(transform) = transform {
            let uniform = (*light, *transform).into();
            lighting.point_lights.buffer.push(uniform);
        } else {
            let uniform = (*light).into();
            lighting.point_lights.buffer.push(uniform);
        }
    }
    lighting.point_lights.buffer.enqueue_update(&device, &queue);
}
