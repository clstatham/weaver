use assets::material_mesh::{LoadedModelWithMaterials, ObjMaterialModelLoader};
use light::{ExtractedPointLight, PointLightPlugin, PointLightUniform};
use material::{BLACK_TEXTURE, ERROR_TEXTURE, MaterialPlugin, WHITE_TEXTURE};
use prelude::Material;
use render::{
    DrawPbr, PbrLightingInformation, PbrPipeline, queue_pbr_meshes, update_environment_map,
};
use skybox::{Skybox, SkyboxPlugin, SkyboxRenderablePlugin};
use weaver_app::prelude::*;
use weaver_asset::{AssetApp, Assets};
use weaver_core::texture::Texture;
use weaver_ecs::{
    component::{Res, ResMut},
    condition::{not, resource_changed, resource_exists},
    prelude::Commands,
    query::Query,
    system::IntoSystemConfig,
};
use weaver_renderer::{
    RenderApp, RenderStage, WgpuDevice, WgpuQueue,
    bind_group::{BindGroup, ResourceBindGroupPlugin},
    phase::{AddDrawFunction, Opaque3d},
    prelude::InitRenderResourcePlugin,
};
use weaver_util::prelude::*;

pub mod assets;
pub mod light;
pub mod material;
pub mod render;
pub mod skybox;

pub mod prelude {
    pub use crate::PbrPlugin;
    pub use crate::assets::material_mesh::*;
    pub use crate::light::*;
    pub use crate::material::*;
    pub use crate::skybox::*;
}

pub struct PbrPlugin;

impl Plugin for PbrPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_asset::<Material>();
        app.add_asset::<LoadedModelWithMaterials>();
        app.add_asset_loader::<ObjMaterialModelLoader, _>();

        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.add_plugin(MaterialPlugin)?;
        render_app.add_plugin(PointLightPlugin)?;
        render_app.add_plugin(SkyboxPlugin)?;
        render_app.add_plugin(SkyboxRenderablePlugin)?;

        render_app.add_plugin(ResourceBindGroupPlugin::<PbrLightingInformation>::default())?;

        render_app.world_mut().add_system(
            init_pbr_lighting_information.run_if(not(resource_exists::<PbrLightingInformation>())),
            RenderStage::Prepare,
        );
        render_app.world_mut().add_system(
            update_environment_map
                .after(init_pbr_lighting_information)
                .run_if(resource_changed::<Skybox>()),
            RenderStage::Prepare,
        );
        render_app.world_mut().add_system(
            update_pbr_lighting_information
                .after(init_pbr_lighting_information)
                .run_if(resource_exists::<PbrLightingInformation>()),
            RenderStage::Prepare,
        );

        render_app.add_plugin(InitRenderResourcePlugin::<PbrPipeline>::default())?;

        render_app.add_draw_function::<Opaque3d, DrawPbr>();
        render_app.world_mut().add_system(
            queue_pbr_meshes.run_if(resource_exists::<BindGroup<PbrLightingInformation>>()),
            RenderStage::Queue,
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

pub(crate) fn init_pbr_lighting_information(commands: Commands) {
    commands.init_resource::<PbrLightingInformation>();
}

pub(crate) fn update_pbr_lighting_information(
    mut lighting: ResMut<PbrLightingInformation>,
    mut lights: Query<&ExtractedPointLight>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) {
    let buffer = &mut lighting.point_lights.buffer;
    buffer.clear();
    for light in lights.iter() {
        buffer.push((*light).into());
    }
    if buffer.is_empty() {
        buffer.push(PointLightUniform::NONE);
    }
    buffer.enqueue_update(&device, &queue);
}
