use weaver_proc_macro::Component;

use crate::renderer::pass::{pbr::PbrRenderPass, Pass};

use super::{color::Color, texture::Texture};

/// PBR material based on Bevy
#[derive(Component)]
pub struct Material {
    pub diffuse: Color,
    pub diffuse_texture: Option<Texture>,
    pub metallic: f32,
    pub normal_texture: Option<Texture>,

    pub texture_scaling: f32,

    pub(crate) bind_group: Option<wgpu::BindGroup>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            diffuse: Color::WHITE,
            diffuse_texture: None,
            metallic: 2.0,
            normal_texture: None,
            texture_scaling: 1.0,
            bind_group: None,
        }
    }
}

impl Material {
    pub fn new(base_color_texture: Option<Texture>, normal_texture: Option<Texture>) -> Self {
        Self {
            diffuse_texture: base_color_texture,
            normal_texture,
            ..Default::default()
        }
    }

    pub fn with_diffuse(mut self, diffuse: Color) -> Self {
        self.diffuse = diffuse;
        self
    }

    pub fn with_diffuse_texture(mut self, diffuse_texture: Texture) -> Self {
        self.diffuse_texture = Some(diffuse_texture);
        self
    }

    pub fn with_normal_texture(mut self, normal_texture: Texture) -> Self {
        self.normal_texture = Some(normal_texture);
        self
    }

    pub fn with_metallic(mut self, metallic: f32) -> Self {
        self.metallic = metallic;
        self
    }

    pub fn has_bind_group(&self) -> bool {
        self.bind_group.is_some()
    }

    pub fn create_bind_group(&mut self, device: &wgpu::Device, render_pass: &PbrRenderPass) {
        let diffuse_texture = self.diffuse_texture.as_ref().unwrap();
        let normal_texture = self.normal_texture.as_ref().unwrap();
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout: &render_pass.bind_group_layout,
            entries: &[
                // model_transform
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(
                        render_pass
                            .model_transform_buffer
                            .as_entire_buffer_binding(),
                    ),
                },
                // camera
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(
                        render_pass.camera_buffer.as_entire_buffer_binding(),
                    ),
                },
                // material
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(
                        render_pass.material_buffer.as_entire_buffer_binding(),
                    ),
                },
                // tex
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                // tex_sampler
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
                // normal_tex
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                },
                // normal_tex_sampler
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&normal_texture.sampler),
                },
                // lights
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Buffer(
                        render_pass.lights_buffer.as_entire_buffer_binding(),
                    ),
                },
            ],
        });
        self.bind_group = Some(bind_group);
    }
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct MaterialUniform {
    pub base_color: [f32; 4],
    pub metallic: [f32; 4],
    pub texture_scaling: [f32; 4],
}

impl From<&Material> for MaterialUniform {
    fn from(material: &Material) -> Self {
        Self {
            base_color: material.diffuse.vec4().into(),
            metallic: [material.metallic; 4],
            texture_scaling: [material.texture_scaling; 4],
        }
    }
}
