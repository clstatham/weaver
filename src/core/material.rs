use std::{path::Path, sync::Arc};

use weaver_proc_macro::Component;

use crate::{
    app::asset_server::AssetId,
    ecs::World,
    renderer::internals::{
        BindGroupLayoutCache, BindableComponent, GpuComponent, GpuHandle, GpuResourceManager,
        GpuResourceType, LazyBindGroup, LazyGpuHandle,
    },
};

use super::{
    color::Color,
    texture::{NormalMapTexture, SdrTexture, Texture, TextureFormat},
};

/// PBR material based on Bevy
#[derive(Clone, Component)]
pub struct Material {
    asset_id: AssetId,
    pub diffuse: Color,
    pub diffuse_texture: Option<SdrTexture>,
    pub metallic: f32,
    pub normal_texture: Option<NormalMapTexture>,
    pub roughness: f32,
    pub roughness_texture: Option<SdrTexture>,
    pub ambient_occlusion_texture: Option<SdrTexture>,

    pub texture_scaling: f32,

    pub(crate) properties_handle: LazyGpuHandle,
    pub(crate) bind_group: LazyBindGroup<Self>,
    pub(crate) sampler: LazyGpuHandle,
}

impl Material {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        diffuse_texture: Option<SdrTexture>,
        normal_texture: Option<NormalMapTexture>,
        roughness_texture: Option<SdrTexture>,
        ambient_occlusion_texture: Option<SdrTexture>,
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
            properties_handle: LazyGpuHandle::new(
                GpuResourceType::Uniform {
                    usage: wgpu::BufferUsages::UNIFORM
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::COPY_SRC,
                    size: std::mem::size_of::<MaterialUniform>(),
                },
                Some("Material"),
                None,
            ),
            bind_group: LazyBindGroup::default(),
            sampler: LazyGpuHandle::new(
                GpuResourceType::Sampler {
                    address_mode: wgpu::AddressMode::Repeat,
                    filter_mode: wgpu::FilterMode::Linear,
                    compare: None,
                },
                Some("Material Sampler"),
                None,
            ),
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
        self.diffuse_texture = Some(SdrTexture::from_texture(diffuse_texture));
        self
    }

    pub fn with_normal_texture(mut self, normal_texture: Texture) -> Self {
        self.normal_texture = Some(NormalMapTexture::from_texture(normal_texture));
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
        self.roughness_texture = Some(SdrTexture::from_texture(roughness_texture));
        self
    }

    pub(crate) fn load_gltf(path: impl AsRef<Path>, id: AssetId) -> anyhow::Result<Vec<Self>> {
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
                        mat.diffuse_texture =
                            Some(SdrTexture::from_texture(Texture::from_data_r8g8b8(
                                image.width,
                                image.height,
                                &image.pixels,
                                SdrTexture::FORMAT,
                                Some("GLTF Material Diffuse Texture"),
                            )));
                    }
                    gltf::image::Format::R8G8B8A8 => {
                        mat.diffuse_texture =
                            Some(SdrTexture::from_texture(Texture::from_data_rgba8(
                                image.width,
                                image.height,
                                &image.pixels,
                                SdrTexture::FORMAT,
                                Some("GLTF Material Diffuse Texture"),
                            )));
                    }
                    _ => {
                        todo!("Unsupported GLTF Texture Format");
                    }
                }
            } else {
                log::warn!("GLTF Material has no diffuse texture");
            }
            if let Some(texture) = material.normal_texture() {
                let image = images.get(texture.texture().source().index()).unwrap();
                match image.format {
                    gltf::image::Format::R8G8B8 => {
                        mat.normal_texture =
                            Some(NormalMapTexture::from_texture(Texture::from_data_r8g8b8(
                                image.width,
                                image.height,
                                &image.pixels,
                                NormalMapTexture::FORMAT,
                                Some("GLTF Material Normal Texture"),
                            )));
                    }
                    gltf::image::Format::R8G8B8A8 => {
                        mat.normal_texture =
                            Some(NormalMapTexture::from_texture(Texture::from_data_rgba8(
                                image.width,
                                image.height,
                                &image.pixels,
                                NormalMapTexture::FORMAT,
                                Some("GLTF Material Normal Texture"),
                            )));
                    }
                    _ => {
                        todo!("Unsupported GLTF Texture Format");
                    }
                }
            } else {
                log::warn!("GLTF Material has no normal texture");
            }
            if let Some(texture) = material
                .pbr_metallic_roughness()
                .metallic_roughness_texture()
            {
                let image = images.get(texture.texture().source().index()).unwrap();
                match image.format {
                    gltf::image::Format::R8G8B8 => {
                        mat.roughness_texture =
                            Some(SdrTexture::from_texture(Texture::from_data_r8g8b8(
                                image.width,
                                image.height,
                                &image.pixels,
                                SdrTexture::FORMAT,
                                Some("GLTF Material Roughness Texture"),
                            )));
                    }
                    gltf::image::Format::R8G8B8A8 => {
                        mat.roughness_texture =
                            Some(SdrTexture::from_texture(Texture::from_data_rgba8(
                                image.width,
                                image.height,
                                &image.pixels,
                                SdrTexture::FORMAT,
                                Some("GLTF Material Roughness Texture"),
                            )));
                    }
                    _ => {
                        todo!("Unsupported GLTF Texture Format");
                    }
                }
            } else {
                log::warn!("GLTF Material has no roughness texture");
            }
            if let Some(ao_texture) = material.occlusion_texture() {
                let image = images.get(ao_texture.texture().source().index()).unwrap();
                match image.format {
                    gltf::image::Format::R8G8B8 => {
                        mat.ambient_occlusion_texture =
                            Some(SdrTexture::from_texture(Texture::from_data_r8g8b8(
                                image.width,
                                image.height,
                                &image.pixels,
                                SdrTexture::FORMAT,
                                Some("GLTF Material Ambient Occlusion Texture"),
                            )));
                    }
                    gltf::image::Format::R8G8B8A8 => {
                        mat.ambient_occlusion_texture =
                            Some(SdrTexture::from_texture(Texture::from_data_rgba8(
                                image.width,
                                image.height,
                                &image.pixels,
                                SdrTexture::FORMAT,
                                Some("GLTF Material Ambient Occlusion Texture"),
                            )));
                    }
                    _ => {
                        todo!("Unsupported GLTF Texture Format");
                    }
                }
            } else {
                log::warn!("GLTF Material has no ambient occlusion texture");
            }

            materials.push(mat);
        }

        Ok(materials)
    }
}

#[derive(Debug, Clone, Copy, Component, bytemuck::Pod, bytemuck::Zeroable)]
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

impl GpuComponent for Material {
    fn lazy_init(&self, manager: &GpuResourceManager) -> anyhow::Result<Vec<GpuHandle>> {
        if let Some(diffuse_texture) = &self.diffuse_texture {
            diffuse_texture.lazy_init(manager)?;
        }
        if let Some(normal_texture) = &self.normal_texture {
            normal_texture.lazy_init(manager)?;
        }
        if let Some(roughness_texture) = &self.roughness_texture {
            roughness_texture.lazy_init(manager)?;
        }
        if let Some(ambient_occlusion_texture) = &self.ambient_occlusion_texture {
            ambient_occlusion_texture.lazy_init(manager)?;
        }

        Ok(vec![self.properties_handle.lazy_init(manager)?])
    }

    fn update_resources(&self, world: &World) -> anyhow::Result<()> {
        self.properties_handle
            .update(&[MaterialUniform::from(self)]);

        if let Some(diffuse_texture) = &self.diffuse_texture {
            diffuse_texture.update_resources(world)?;
        }
        if let Some(normal_texture) = &self.normal_texture {
            normal_texture.update_resources(world)?;
        }
        if let Some(roughness_texture) = &self.roughness_texture {
            roughness_texture.update_resources(world)?;
        }
        if let Some(ambient_occlusion_texture) = &self.ambient_occlusion_texture {
            ambient_occlusion_texture.update_resources(world)?;
        }

        Ok(())
    }

    fn destroy_resources(&self) -> anyhow::Result<()> {
        self.properties_handle.destroy();

        if let Some(diffuse_texture) = &self.diffuse_texture {
            diffuse_texture.destroy_resources()?;
        }
        if let Some(normal_texture) = &self.normal_texture {
            normal_texture.destroy_resources()?;
        }
        if let Some(roughness_texture) = &self.roughness_texture {
            roughness_texture.destroy_resources()?;
        }
        if let Some(ambient_occlusion_texture) = &self.ambient_occlusion_texture {
            ambient_occlusion_texture.destroy_resources()?;
        }

        Ok(())
    }
}

impl BindableComponent for Material {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        let properties_binding = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };

        let diffuse_texture_binding = wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };

        let diffuse_texture_sampler_binding = wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };

        let normal_texture_binding = wgpu::BindGroupLayoutEntry {
            binding: 3,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };

        let normal_texture_sampler_binding = wgpu::BindGroupLayoutEntry {
            binding: 4,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };

        let roughness_texture_binding = wgpu::BindGroupLayoutEntry {
            binding: 5,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };

        let roughness_texture_sampler_binding = wgpu::BindGroupLayoutEntry {
            binding: 6,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };

        let ambient_occlusion_texture_binding = wgpu::BindGroupLayoutEntry {
            binding: 7,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };

        let ambient_occlusion_texture_sampler_binding = wgpu::BindGroupLayoutEntry {
            binding: 8,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };

        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Material Bind Group Layout"),
            entries: &[
                properties_binding,
                diffuse_texture_binding,
                diffuse_texture_sampler_binding,
                normal_texture_binding,
                normal_texture_sampler_binding,
                roughness_texture_binding,
                roughness_texture_sampler_binding,
                ambient_occlusion_texture_binding,
                ambient_occlusion_texture_sampler_binding,
            ],
        })
    }

    fn create_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        let properties_handle = self.properties_handle.lazy_init(manager)?;

        let sampler_handle = self.sampler.lazy_init(manager)?;
        let sampler = sampler_handle.get_sampler().unwrap();

        let diffuse_texture = if let Some(diffuse_texture) = self.diffuse_texture.clone() {
            diffuse_texture
        } else {
            SdrTexture::from_texture(Texture::default_texture())
        };
        let diffuse_texture = diffuse_texture.lazy_init(manager)?;
        let diffuse_texture = diffuse_texture.first().unwrap();
        let diffuse_texture = diffuse_texture.get_texture().unwrap();
        let diffuse_texture_view = diffuse_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Material Diffuse Texture View"),
            format: Some(SdrTexture::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..Default::default()
        });

        let normal_texture = if let Some(normal_texture) = self.normal_texture.clone() {
            normal_texture
        } else {
            NormalMapTexture::from_texture(Texture::default_texture())
        };
        let normal_texture = normal_texture.lazy_init(manager)?;
        let normal_texture = normal_texture.first().unwrap();
        let normal_texture = normal_texture.get_texture().unwrap();
        let normal_texture_view = normal_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Material Normal Texture View"),
            format: Some(NormalMapTexture::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..Default::default()
        });

        let roughness_texture = if let Some(roughness_texture) = self.roughness_texture.clone() {
            roughness_texture
        } else {
            SdrTexture::from_texture(Texture::default_texture())
        };
        let roughness_texture = roughness_texture.lazy_init(manager)?;
        let roughness_texture = roughness_texture.first().unwrap();
        let roughness_texture = roughness_texture.get_texture().unwrap();
        let roughness_texture_view = roughness_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Material Roughness Texture View"),
            format: Some(SdrTexture::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..Default::default()
        });

        let ambient_occlusion_texture =
            if let Some(ambient_occlusion_texture) = self.ambient_occlusion_texture.clone() {
                ambient_occlusion_texture
            } else {
                SdrTexture::from_texture(Texture::default_texture())
            };
        let ambient_occlusion_texture = ambient_occlusion_texture.lazy_init(manager)?;
        let ambient_occlusion_texture = ambient_occlusion_texture.first().unwrap();
        let ambient_occlusion_texture = ambient_occlusion_texture.get_texture().unwrap();
        let ambient_occlusion_texture_view =
            ambient_occlusion_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Material Ambient Occlusion Texture View"),
                format: Some(SdrTexture::FORMAT),
                dimension: Some(wgpu::TextureViewDimension::D2),
                ..Default::default()
            });

        let layout = cache.get_or_create::<Self>(manager.device());
        let bind_group = manager
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Material Bind Group"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(
                            properties_handle
                                .get_buffer()
                                .unwrap()
                                .as_entire_buffer_binding(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&normal_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&roughness_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: wgpu::BindingResource::TextureView(
                            &ambient_occlusion_texture_view,
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });

        Ok(Arc::new(bind_group))
    }

    fn bind_group(&self) -> Option<Arc<wgpu::BindGroup>> {
        self.bind_group.bind_group().clone()
    }

    fn lazy_init_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &crate::renderer::internals::BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        if let Some(bind_group) = self.bind_group.bind_group() {
            return Ok(bind_group);
        }

        let bind_group = self.bind_group.lazy_init_bind_group(manager, cache, self)?;
        Ok(bind_group)
    }
}
