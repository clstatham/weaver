use std::{fmt::Debug, path::Path};

use weaver_ecs::prelude::*;
use weaver_proc_macro::{BindableComponent, Component, GpuComponent};

use crate::{
    asset_server::AssetId,
    renderer::internals::{GpuResourceType, LazyBindGroup, LazyGpuHandle},
};

use super::{
    color::Color,
    texture::{NormalMapTexture, SdrTexture, Texture, TextureFormat},
};

/// PBR material based on Bevy
#[derive(Clone, Component, GpuComponent, BindableComponent)]
#[gpu(update = "update")]
pub struct Material {
    asset_id: AssetId,

    pub diffuse: Color,
    pub metallic: f32,
    pub roughness: f32,
    pub texture_scaling: f32,

    #[uniform]
    pub(crate) properties_handle: LazyGpuHandle,

    #[gpu(component)]
    #[texture(format = Rgba8UnormSrgb, sample_type = filterable_float, view_dimension = D2, default = SdrTexture::default)]
    pub(crate) diffuse_texture: Option<SdrTexture>,

    #[sampler(filtering = true)]
    pub(crate) diffuse_sampler: LazyGpuHandle,

    #[gpu(component)]
    #[texture(format = Rgba8Unorm, sample_type = filterable_float, view_dimension = D2, default = NormalMapTexture::default)]
    pub(crate) normal_texture: Option<NormalMapTexture>,

    #[sampler(filtering = true)]
    pub(crate) normal_sampler: LazyGpuHandle,

    #[gpu(component)]
    #[texture(format = Rgba8UnormSrgb, sample_type = filterable_float, view_dimension = D2, default = SdrTexture::default)]
    pub(crate) roughness_texture: Option<SdrTexture>,

    #[sampler(filtering = true)]
    pub(crate) roughness_sampler: LazyGpuHandle,

    #[gpu(component)]
    #[texture(format = Rgba8UnormSrgb, sample_type = filterable_float, view_dimension = D2, default = SdrTexture::default)]
    pub(crate) ambient_occlusion_texture: Option<SdrTexture>,

    #[sampler(filtering = true)]
    pub(crate) ambient_occlusion_sampler: LazyGpuHandle,

    pub(crate) bind_group: LazyBindGroup<Self>,
}

impl Debug for Material {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Material")
            .field("diffuse", &self.diffuse)
            .field("metallic", &self.metallic)
            .field("roughness", &self.roughness)
            .field("texture_scaling", &self.texture_scaling)
            .finish()
    }
}

impl Material {
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
            properties_handle: Self::new_properties_handle(),
            bind_group: LazyBindGroup::default(),
            diffuse_sampler: Self::new_sampler_handle(),
            normal_sampler: Self::new_sampler_handle(),
            roughness_sampler: Self::new_sampler_handle(),
            ambient_occlusion_sampler: Self::new_sampler_handle(),
        }
    }

    #[doc(hidden)]
    fn new_properties_handle() -> LazyGpuHandle {
        LazyGpuHandle::new(
            GpuResourceType::Uniform {
                usage: wgpu::BufferUsages::UNIFORM
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                size: std::mem::size_of::<MaterialUniform>(),
            },
            Some("Material"),
            None,
        )
    }

    #[doc(hidden)]
    fn new_sampler_handle() -> LazyGpuHandle {
        LazyGpuHandle::new(
            GpuResourceType::Sampler {
                address_mode: wgpu::AddressMode::Repeat,
                filter_mode: wgpu::FilterMode::Linear,
                compare: None,
            },
            Some("Material Sampler"),
            None,
        )
    }

    pub fn asset_id(&self) -> &AssetId {
        &self.asset_id
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
                id.clone(),
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

    fn update(&self, _world: &World) -> anyhow::Result<()> {
        self.properties_handle
            .update(&[MaterialUniform::from(self)]);

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Component, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct MaterialUniform {
    pub base_color: glam::Vec4,
    pub properties: glam::Vec4, // metallic, roughness, 0, 0
    pub texture_scaling: glam::Vec4,
}

impl From<&Material> for MaterialUniform {
    fn from(material: &Material) -> Self {
        Self {
            base_color: material.diffuse.vec4(),
            properties: [material.metallic, material.roughness, 0.0, 0.0].into(),
            texture_scaling: [material.texture_scaling; 4].into(),
        }
    }
}
