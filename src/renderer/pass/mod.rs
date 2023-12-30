use weaver_ecs::World;

use crate::core::texture::Texture;

pub mod hdr;
pub mod pbr;

pub fn preprocess_shader(shader: &str) -> String {
    let mut output = String::new();
    output.push_str(include_str!("common.wgsl"));
    output.push('\n');
    output.push_str(shader);
    output
}

#[macro_export]
macro_rules! include_shader {
    ($name:literal) => {
        $crate::renderer::pass::preprocess_shader(include_str!($name))
    };
}

pub trait Pass {
    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_texture: &Texture,
        normal_texture: &Texture,
        depth_texture: &Texture,
        world: &World,
    ) -> anyhow::Result<()>;

    fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized;

    fn pipeline_layout(device: &wgpu::Device) -> wgpu::PipelineLayout
    where
        Self: Sized;

    fn pipeline(&self) -> &wgpu::RenderPipeline;
}
