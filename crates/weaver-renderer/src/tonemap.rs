//! Tonemapping: resolving a view's HDR target onto the window.

use weaver_app::{App, plugin::Plugin};
use weaver_ecs::{prelude::World, world::ConstructFromWorld};
use weaver_util::prelude::*;

use crate::{
    WgpuDevice,
    bind_group::BindGroupLayouts,
    pipeline::{InitRenderResourcePlugin, RenderPipeline, RenderPipelineDescriptor},
    shader::ShaderLibrary,
    texture::texture_format,
    view::ViewTarget,
};

/// Draws a fullscreen triangle sampling a view's HDR target (its `BindGroup<ViewTarget>`) into the
/// swapchain format.
pub struct TonemapPipeline {
    pub pipeline: RenderPipeline,
}

impl ConstructFromWorld for TonemapPipeline {
    fn from_world(world: &World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();
        let layouts = world.get_resource::<BindGroupLayouts>().unwrap();
        let shaders = world.get_resource::<ShaderLibrary>().unwrap();
        let shader = shaders.load(&device, "hdr.wesl");

        let pipeline = RenderPipelineDescriptor::new("Tonemap Pipeline", &shader)
            .bind_group_layouts([layouts.get::<ViewTarget>(&device)])
            .color_format(texture_format::VIEW_FORMAT)
            .build(&device);

        Self { pipeline }
    }
}

pub struct TonemapPlugin;

impl Plugin for TonemapPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(InitRenderResourcePlugin::<TonemapPipeline>::default())?;
        Ok(())
    }
}
