use weaver_proc_macro::Component;

use crate::renderer::pass::{pbr::PbrRenderPass, Pass};

use super::{color::Color, texture::Texture};

/// PBR material based on Bevy
#[derive(Component)]
pub struct Material {
    pub base_color: Color,
    pub base_color_texture: Option<Texture>,
    pub metallic: f32,
    pub normal_texture: Option<Texture>,
    pub(crate) bind_group: Option<wgpu::BindGroup>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            base_color: Color::WHITE,
            base_color_texture: None,
            metallic: 2.0,
            normal_texture: None,
            bind_group: None,
        }
    }
}

impl Material {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_color(mut self, base_color: Color) -> Self {
        self.base_color = base_color;
        self
    }

    pub fn with_base_color_texture(mut self, base_color_texture: Texture) -> Self {
        self.base_color_texture = Some(base_color_texture);
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
        let base_color_texture = self.base_color_texture.as_ref().unwrap();
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
                    resource: wgpu::BindingResource::TextureView(&base_color_texture.view),
                },
                // tex_sampler
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&base_color_texture.sampler),
                },
                // lights
                wgpu::BindGroupEntry {
                    binding: 5,
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
    pub metallic: f32,
    _padding: [u32; 3],
}

impl From<&Material> for MaterialUniform {
    fn from(material: &Material) -> Self {
        Self {
            base_color: material.base_color.vec4().into(),
            metallic: material.metallic,
            _padding: [0; 3],
        }
    }
}
