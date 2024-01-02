use std::{path::Path, sync::Arc};

use weaver_proc_macro::Component;

use crate::{app::asset_server::AssetId, renderer::pass::pbr::PbrRenderPass};

use super::{color::Color, texture::Texture};

/// PBR material based on Bevy
#[derive(Clone, Component)]
pub struct Material {
    asset_id: AssetId,
    pub diffuse: Color,
    pub diffuse_texture: Option<Texture>,
    pub metallic: f32,
    pub normal_texture: Option<Texture>,
    pub roughness: f32,
    pub roughness_texture: Option<Texture>,
    pub ambient_occlusion_texture: Option<Texture>,

    pub texture_scaling: f32,

    bind_group: Option<Arc<wgpu::BindGroup>>,
}

impl Material {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        diffuse_texture: Option<Texture>,
        normal_texture: Option<Texture>,
        roughness_texture: Option<Texture>,
        ambient_occlusion_texture: Option<Texture>,
        metallic: Option<f32>,
        roughness: Option<f32>,
        texture_scaling: Option<f32>,
        asset_id: AssetId,
    ) -> Self {
        Self {
            asset_id,
            diffuse_texture,
            normal_texture,
            roughness_texture,
            ambient_occlusion_texture,
            texture_scaling: texture_scaling.unwrap_or(1.0),
            diffuse: Color::WHITE,
            metallic: metallic.unwrap_or(1.0),
            roughness: roughness.unwrap_or(1.0),
            bind_group: None,
        }
    }

    pub fn asset_id(&self) -> AssetId {
        self.asset_id
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

    pub fn with_roughness(mut self, roughness: f32) -> Self {
        self.roughness = roughness;
        self
    }

    pub fn with_roughness_texture(mut self, roughness_texture: Texture) -> Self {
        self.roughness_texture = Some(roughness_texture);
        self
    }

    pub fn has_bind_group(&self) -> bool {
        self.bind_group.is_some()
    }

    pub fn bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.bind_group.as_ref().map(|b| b.as_ref())
    }

    pub fn create_bind_group(&mut self, device: &wgpu::Device, render_pass: &PbrRenderPass) {
        let diffuse_texture = self.diffuse_texture.as_ref().unwrap();
        let normal_texture = self.normal_texture.as_ref().unwrap();
        let roughness_texture = self.roughness_texture.as_ref().unwrap();
        let ambient_occlusion_texture = self.ambient_occlusion_texture.as_ref().unwrap();
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
                    resource: wgpu::BindingResource::TextureView(diffuse_texture.view()),
                },
                // tex_sampler
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(diffuse_texture.sampler()),
                },
                // normal_tex
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(normal_texture.view()),
                },
                // normal_tex_sampler
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(normal_texture.sampler()),
                },
                // roughness_tex
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(roughness_texture.view()),
                },
                // roughness_tex_sampler
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Sampler(roughness_texture.sampler()),
                },
                // ambient occlusion texture
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(ambient_occlusion_texture.view()),
                },
                // ambient occlusion texture sampler
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::Sampler(ambient_occlusion_texture.sampler()),
                },
                // point lights
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::Buffer(
                        render_pass.point_light_buffer.as_entire_buffer_binding(),
                    ),
                },
                // directional lights
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::Buffer(
                        render_pass
                            .directional_light_buffer
                            .as_entire_buffer_binding(),
                    ),
                },
            ],
        });
        self.bind_group = Some(Arc::new(bind_group));
    }

    pub(crate) fn load_gltf(
        path: impl AsRef<Path>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: AssetId,
    ) -> anyhow::Result<Vec<Self>> {
        let (document, _buffers, images) = gltf::import(path.as_ref())?;
        let mut materials = Vec::new();

        for material in document.materials() {
            let metallic = material.pbr_metallic_roughness().metallic_factor();
            let roughness = material.pbr_metallic_roughness().roughness_factor();
            let mut mat = Self::new(
                None,
                None,
                None,
                None,
                Some(metallic),
                Some(roughness),
                None,
                id,
            );
            if let Some(texture) = material.pbr_metallic_roughness().base_color_texture() {
                let image = images.get(texture.texture().source().index()).unwrap();
                match image.format {
                    gltf::image::Format::R8G8B8 => {
                        mat.diffuse_texture = Some(Texture::from_data_r8g8b8(
                            image.width as usize,
                            image.height as usize,
                            &image.pixels,
                            device,
                            queue,
                            Some("GLTF Mesh Diffuse Texture"),
                            false,
                        ));
                    }
                    gltf::image::Format::R8G8B8A8 => {
                        mat.diffuse_texture = Some(Texture::from_data_rgba8(
                            image.width as usize,
                            image.height as usize,
                            &image.pixels,
                            device,
                            queue,
                            Some("GLTF Mesh Diffuse Texture"),
                            false,
                        ));
                    }
                    _ => {
                        todo!("Unsupported GLTF Texture Format");
                    }
                }
            } else {
                log::warn!("GLTF Mesh has no diffuse texture");
                mat.diffuse_texture = Some(Texture::default_texture(device, queue));
            }
            if let Some(texture) = material.normal_texture() {
                let image = images.get(texture.texture().source().index()).unwrap();
                match image.format {
                    gltf::image::Format::R8G8B8 => {
                        mat.normal_texture = Some(Texture::from_data_r8g8b8(
                            image.width as usize,
                            image.height as usize,
                            &image.pixels,
                            device,
                            queue,
                            Some("GLTF Mesh Normal Texture"),
                            true,
                        ));
                    }
                    gltf::image::Format::R8G8B8A8 => {
                        mat.normal_texture = Some(Texture::from_data_rgba8(
                            image.width as usize,
                            image.height as usize,
                            &image.pixels,
                            device,
                            queue,
                            Some("GLTF Mesh Normal Texture"),
                            true,
                        ));
                    }
                    _ => {
                        todo!("Unsupported GLTF Texture Format");
                    }
                }
            } else {
                log::warn!("GLTF Mesh has no normal texture");
            }
            if let Some(texture) = material
                .pbr_metallic_roughness()
                .metallic_roughness_texture()
            {
                let image = images.get(texture.texture().source().index()).unwrap();
                match image.format {
                    gltf::image::Format::R8G8B8 => {
                        mat.roughness_texture = Some(Texture::from_data_r8g8b8(
                            image.width as usize,
                            image.height as usize,
                            &image.pixels,
                            device,
                            queue,
                            Some("GLTF Mesh Roughness Texture"),
                            false,
                        ));
                    }
                    gltf::image::Format::R8G8B8A8 => {
                        mat.roughness_texture = Some(Texture::from_data_rgba8(
                            image.width as usize,
                            image.height as usize,
                            &image.pixels,
                            device,
                            queue,
                            Some("GLTF Mesh Roughness Texture"),
                            false,
                        ));
                    }
                    _ => {
                        todo!("Unsupported GLTF Texture Format");
                    }
                }
            } else {
                log::warn!("GLTF Mesh has no roughness texture");
            }
            if let Some(ao_texture) = material.occlusion_texture() {
                let image = images.get(ao_texture.texture().source().index()).unwrap();
                match image.format {
                    gltf::image::Format::R8G8B8 => {
                        mat.ambient_occlusion_texture = Some(Texture::from_data_r8g8b8(
                            image.width as usize,
                            image.height as usize,
                            &image.pixels,
                            device,
                            queue,
                            Some("GLTF Mesh Ambient Occlusion Texture"),
                            false,
                        ));
                    }
                    gltf::image::Format::R8G8B8A8 => {
                        mat.ambient_occlusion_texture = Some(Texture::from_data_rgba8(
                            image.width as usize,
                            image.height as usize,
                            &image.pixels,
                            device,
                            queue,
                            Some("GLTF Mesh Ambient Occlusion Texture"),
                            false,
                        ));
                    }
                    _ => {
                        todo!("Unsupported GLTF Texture Format");
                    }
                }
            } else {
                log::warn!("GLTF Mesh has no ambient occlusion texture");
            }

            materials.push(mat);
        }

        Ok(materials)
    }
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct MaterialUniform {
    pub base_color: [f32; 4],
    pub properties: [f32; 4], // metallic, roughness, 0, 0
    pub texture_scaling: [f32; 4],
}

impl From<&Material> for MaterialUniform {
    fn from(material: &Material) -> Self {
        Self {
            base_color: material.diffuse.vec4().into(),
            properties: [material.metallic, material.roughness, 0.0, 0.0],
            texture_scaling: [material.texture_scaling; 4],
        }
    }
}
