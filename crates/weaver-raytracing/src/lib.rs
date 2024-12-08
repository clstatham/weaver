use render::RaytracingOutput;
use weaver_app::{plugin::Plugin, App};
use weaver_asset::AssetApp;
use weaver_ecs::system::IntoSystemConfig;
use weaver_renderer::{
    clear_color::render_clear_color,
    hdr::render_hdr,
    prelude::{RenderPipelinePlugin, ResourceBindGroupPlugin},
    RenderApp, RenderStage,
};
use weaver_util::prelude::Result;

pub mod geometry;
pub mod light;
pub mod material;
pub mod render;

pub struct RaytracingPlugin;

impl Plugin for RaytracingPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_asset::<material::Material>();

        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.add_plugin(RaytracingRenderPlugin)?;
        Ok(())
    }
}

pub struct RaytracingRenderPlugin;

impl Plugin for RaytracingRenderPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_plugin(light::PointLightPlugin)?;

        render_app.add_plugin(ResourceBindGroupPlugin::<
            light::RaytracingLightingInformation,
        >::default())?;

        render_app
            .add_plugin(ResourceBindGroupPlugin::<render::GpuObjectRaytracingBuffer>::default())?;

        render_app
            .add_plugin(RenderPipelinePlugin::<render::RaytracingRenderPipeline>::default())?;

        render_app.add_system(
            render::extract_gpu_object_raytracing_buffer,
            RenderStage::Extract,
        );

        render_app.add_system(
            render::init_raytracing_output,
            RenderStage::ExtractBindGroup,
        );

        render_app.add_system(
            light::init_raytracing_lighting_information,
            RenderStage::InitRenderResources,
        );
        render_app.add_system(
            light::update_raytracing_lighting_information,
            RenderStage::Extract,
        );

        render_app.add_plugin(ResourceBindGroupPlugin::<RaytracingOutput>::default())?;

        render_app.add_system(
            render::init_gpu_object_raytracing_buffer,
            RenderStage::InitRenderResources,
        );

        render_app.add_system(
            render::render_raytracing
                .before(render_hdr)
                .after(render_clear_color),
            RenderStage::Render,
        );

        Ok(())
    }
}
