use std::path::Path;

use weaver_proc_macro::Bundle;

use crate::renderer::pass::{model::ModelRenderPass, Pass};

use super::{mesh::Mesh, texture::Texture, transform::Transform};

#[derive(Bundle)]
pub struct Model {
    pub mesh: Mesh,
    pub transform: Transform,
}

impl Model {
    pub fn new(mesh: Mesh, transform: Transform) -> Self {
        Self { mesh, transform }
    }

    pub fn load_gltf(
        path: impl AsRef<Path>,
        renderer: &crate::renderer::Renderer,
    ) -> anyhow::Result<Self> {
        let mesh = Mesh::load_gltf(
            path,
            &renderer.device,
            &renderer.queue,
            &renderer.model_pass.model_buffer,
            &renderer.model_pass.view_buffer,
            &renderer.model_pass.proj_buffer,
        )?;
        let transform = Transform::new();

        Ok(Self::new(mesh, transform))
    }

    pub fn bind_group(
        device: &wgpu::Device,
        model_buffer: &wgpu::Buffer,
        view_buffer: &wgpu::Buffer,
        proj_buffer: &wgpu::Buffer,
        texture_view: &wgpu::TextureView,
        texture_sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Model Bind Group"),
            layout: &ModelRenderPass::bind_group_layout(device),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: model_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: view_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: proj_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(texture_sampler),
                },
            ],
        })
    }
}
