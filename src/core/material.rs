use std::{path::Path, sync::Arc};

use weaver_proc_macro::Component;

use crate::{
    app::asset_server::AssetId,
    renderer::{
        AllocBuffers, BufferBindingType, BufferHandle, CreateBindGroupLayout, LazyBufferHandle,
        Renderer,
    },
};

use super::{
    color::Color,
    texture::{NormalMapFormat, SdrFormat, Texture, TextureFormat},
};

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

    pub(crate) properties_handle: LazyBufferHandle,
    pub(crate) bind_group: Option<Arc<wgpu::BindGroup>>,
    pub(crate) sampler: Option<Arc<wgpu::Sampler>>,
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
            properties_handle: LazyBufferHandle::new(
                BufferBindingType::Uniform {
                    usage: wgpu::BufferUsages::UNIFORM
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::COPY_SRC,
                    size: Some(std::mem::size_of::<MaterialUniform>()),
                },
                Some("Material"),
                None,
            ),
            bind_group: None,
            sampler: None,
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
                        mat.diffuse_texture = Some(Texture::from_data_r8g8b8(
                            image.width,
                            image.height,
                            &image.pixels,
                            SdrFormat::FORMAT,
                            Some("GLTF Material Diffuse Texture"),
                        ));
                    }
                    gltf::image::Format::R8G8B8A8 => {
                        mat.diffuse_texture = Some(Texture::from_data_rgba8(
                            image.width,
                            image.height,
                            &image.pixels,
                            SdrFormat::FORMAT,
                            Some("GLTF Material Diffuse Texture"),
                        ));
                    }
                    _ => {
                        todo!("Unsupported GLTF Texture Format");
                    }
                }
            } else {
                log::warn!("GLTF Material has no diffuse texture");
                mat.diffuse_texture = Some(Texture::default_texture());
            }
            if let Some(texture) = material.normal_texture() {
                let image = images.get(texture.texture().source().index()).unwrap();
                match image.format {
                    gltf::image::Format::R8G8B8 => {
                        mat.normal_texture = Some(Texture::from_data_r8g8b8(
                            image.width,
                            image.height,
                            &image.pixels,
                            NormalMapFormat::FORMAT,
                            Some("GLTF Material Normal Texture"),
                        ));
                    }
                    gltf::image::Format::R8G8B8A8 => {
                        mat.normal_texture = Some(Texture::from_data_rgba8(
                            image.width,
                            image.height,
                            &image.pixels,
                            NormalMapFormat::FORMAT,
                            Some("GLTF Material Normal Texture"),
                        ));
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
                        mat.roughness_texture = Some(Texture::from_data_r8g8b8(
                            image.width,
                            image.height,
                            &image.pixels,
                            SdrFormat::FORMAT,
                            Some("GLTF Material Roughness Texture"),
                        ));
                    }
                    gltf::image::Format::R8G8B8A8 => {
                        mat.roughness_texture = Some(Texture::from_data_rgba8(
                            image.width,
                            image.height,
                            &image.pixels,
                            SdrFormat::FORMAT,
                            Some("GLTF Material Roughness Texture"),
                        ));
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
                        mat.ambient_occlusion_texture = Some(Texture::from_data_r8g8b8(
                            image.width,
                            image.height,
                            &image.pixels,
                            SdrFormat::FORMAT,
                            Some("GLTF Material Ambient Occlusion Texture"),
                        ));
                    }
                    gltf::image::Format::R8G8B8A8 => {
                        mat.ambient_occlusion_texture = Some(Texture::from_data_rgba8(
                            image.width,
                            image.height,
                            &image.pixels,
                            SdrFormat::FORMAT,
                            Some("GLTF Material Ambient Occlusion Texture"),
                        ));
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

    pub(crate) fn update(&self, renderer: &Renderer) -> anyhow::Result<()> {
        let mut properties_buffer = self
            .properties_handle
            .get_or_create::<MaterialUniform>(renderer);
        let properties = MaterialUniform::from(self);
        properties_buffer.update(&[properties]);

        Ok(())
    }

    pub(crate) fn create_bind_group(
        &mut self,
        renderer: &Renderer,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        if let Some(bind_group) = &self.bind_group {
            return Ok(bind_group.clone());
        }

        let device = &renderer.device;

        let handles = self.alloc_buffers(renderer)?;
        let properties_handle = &handles[0];
        let diffuse_texture_handle = &handles[1];
        let normal_texture_handle = &handles[2];
        let roughness_texture_handle = &handles[3];
        let ambient_occlusion_texture_handle = &handles[4];

        let properties_buffer = properties_handle.get_buffer().unwrap();
        let diffuse_texture = diffuse_texture_handle.get_texture().unwrap();
        let normal_texture = normal_texture_handle.get_texture().unwrap();
        let roughness_texture = roughness_texture_handle.get_texture().unwrap();
        let ambient_occlusion_texture = ambient_occlusion_texture_handle.get_texture().unwrap();

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Material Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let sampler = Arc::new(sampler);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout: &renderer
                .bind_group_layout_cache
                .get_or_create::<Material>(device),
            entries: &[
                // Material properties
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(
                        properties_buffer.as_entire_buffer_binding(),
                    ),
                },
                // Diffuse texture
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &diffuse_texture.create_view(&Default::default()),
                    ),
                },
                // Normal texture
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &normal_texture.create_view(&Default::default()),
                    ),
                },
                // Roughness texture
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(
                        &roughness_texture.create_view(&Default::default()),
                    ),
                },
                // Ambient occlusion texture
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(
                        &ambient_occlusion_texture.create_view(&Default::default()),
                    ),
                },
                // Sampler
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(sampler.as_ref()),
                },
            ],
        });

        let bind_group = Arc::new(bind_group);

        self.bind_group = Some(bind_group.clone());
        self.sampler = Some(sampler);

        Ok(bind_group)
    }
}

#[derive(Debug, Clone, Copy, Component, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct MaterialUniform {
    pub base_color: [f32; 4],
    pub properties: [f32; 4], // metallic, roughness, 0, 0
    pub texture_scaling: [f32; 4],
}

impl CreateBindGroupLayout for MaterialUniform {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Material Uniform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }
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

impl AllocBuffers for Material {
    fn alloc_buffers(&self, renderer: &Renderer) -> anyhow::Result<Vec<BufferHandle>> {
        let mut handles = vec![self
            .properties_handle
            .get_or_create_init::<_, MaterialUniform>(renderer, &[MaterialUniform::from(self)])];
        let diffuse_texture = self.diffuse_texture.as_ref().unwrap();
        handles.push(diffuse_texture.handle.get_or_create::<SdrFormat>(renderer));
        let normal_texture = self.normal_texture.as_ref().unwrap();
        handles.push(
            normal_texture
                .handle
                .get_or_create::<NormalMapFormat>(renderer),
        );
        let roughness_texture = self.roughness_texture.as_ref().unwrap();
        handles.push(
            roughness_texture
                .handle
                .get_or_create::<SdrFormat>(renderer),
        );
        let ambient_occlusion_texture = self.ambient_occlusion_texture.as_ref().unwrap();
        handles.push(
            ambient_occlusion_texture
                .handle
                .get_or_create::<SdrFormat>(renderer),
        );
        Ok(handles)
    }
}

impl CreateBindGroupLayout for Material {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Material Bind Group Layout"),
            entries: &[
                // Material properties
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Diffuse texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Normal texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Roughness texture
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Ambient occlusion texture
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }
}
