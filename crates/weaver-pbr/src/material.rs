use encase::ShaderType;
use weaver_app::{plugin::Plugin, App};
use weaver_asset::{loading::LoadCtx, prelude::*, AssetId};
use weaver_core::{color::Color, texture::Texture};
use weaver_ecs::{
    component::{Res, ResMut},
    prelude::{Reflect, Resource},
};
use weaver_renderer::{
    asset::{ExtractRenderAssetPlugin, RenderAsset},
    bind_group::{AssetBindGroupPlugin, BindGroupLayout, CreateBindGroup},
    buffer::GpuBufferVec,
    extract::Extract,
    prelude::*,
    texture::{texture_format, GpuTexture},
};
use weaver_util::prelude::*;

pub const WHITE_TEXTURE: Handle<Texture> = Handle::from_raw(AssetId::from_u64(
    171952135557955961317447623731106286307u128 as u64,
));
pub const BLACK_TEXTURE: Handle<Texture> = Handle::from_raw(AssetId::from_u64(
    29903794803500143808416926793703205514u128 as u64,
));

#[derive(Reflect, Asset)]
#[reflect(ReflectAsset)]
pub struct Material {
    pub diffuse: Color,
    #[reflect(ignore)]
    pub diffuse_texture: Handle<Texture>,

    #[reflect(ignore)]
    pub normal_texture: Handle<Texture>,

    pub metallic: f32,
    pub roughness: f32,
    #[reflect(ignore)]
    pub metallic_roughness_texture: Handle<Texture>,

    pub ao: f32,
    #[reflect(ignore)]
    pub ao_texture: Handle<Texture>,

    pub texture_scale: f32,
}

impl From<Color> for Material {
    fn from(color: Color) -> Self {
        Self {
            diffuse: color,
            diffuse_texture: WHITE_TEXTURE,
            normal_texture: BLACK_TEXTURE,
            metallic: 0.0,
            roughness: 0.0,
            metallic_roughness_texture: WHITE_TEXTURE,
            ao: 0.0,
            ao_texture: WHITE_TEXTURE,
            texture_scale: 1.0,
        }
    }
}

impl From<Handle<Texture>> for Material {
    fn from(texture: Handle<Texture>) -> Self {
        Self {
            diffuse: Color::WHITE,
            diffuse_texture: texture,
            normal_texture: BLACK_TEXTURE,
            metallic: 0.0,
            roughness: 0.0,
            metallic_roughness_texture: WHITE_TEXTURE,
            ao: 0.0,
            ao_texture: WHITE_TEXTURE,
            texture_scale: 1.0,
        }
    }
}

#[derive(Resource, Default)]
pub struct GltfMaterialLoader;

impl LoadAsset<Material> for GltfMaterialLoader {
    type Param = ResMut<'static, Assets<Texture>>;
    fn load(&self, mut textures: ResMut<Assets<Texture>>, ctx: &mut LoadCtx) -> Result<Material> {
        let bytes = ctx.read_original()?;
        let (document, _buffers, images) = gltf::import_slice(bytes)?;
        if document.materials().count() != 1 {
            bail!("Material file must contain exactly one material");
        }

        let material = document.materials().next().unwrap();
        let metallic = material.pbr_metallic_roughness().metallic_factor();
        let roughness = material.pbr_metallic_roughness().roughness_factor();
        let ao = material
            .occlusion_texture()
            .map_or(1.0, |info| info.strength());
        let diffuse = material.pbr_metallic_roughness().base_color_factor();
        let diffuse_texture = material
            .pbr_metallic_roughness()
            .base_color_texture()
            .map(|info| images[info.texture().index()].clone());
        let normal_texture = material
            .normal_texture()
            .map(|info| images[info.texture().index()].clone());
        let metallic_roughness_texture = material
            .pbr_metallic_roughness()
            .metallic_roughness_texture()
            .map(|info| images[info.texture().index()].clone());
        let ao_texture = material
            .occlusion_texture()
            .map(|info| images[info.texture().index()].clone());

        let diffuse_texture =
            diffuse_texture.ok_or_else(|| anyhow!("Material must have a diffuse texture"))?;
        let normal_texture =
            normal_texture.ok_or_else(|| anyhow!("Material must have a normal texture"))?;
        let metallic_roughness_texture = metallic_roughness_texture
            .ok_or_else(|| anyhow!("Material must have a metallic roughness texture"))?;
        let ao_texture = ao_texture.ok_or_else(|| anyhow!("Material must have an AO texture"))?;

        let diffuse_texture = match diffuse_texture.format {
            gltf::image::Format::R8G8B8 => Texture::from_rgb8(
                &diffuse_texture.pixels,
                diffuse_texture.width,
                diffuse_texture.height,
            ),
            gltf::image::Format::R8G8B8A8 => Texture::from_rgba8(
                &diffuse_texture.pixels,
                diffuse_texture.width,
                diffuse_texture.height,
            ),
            format => bail!(
                "Diffuse texture must be in RGB8 or RGBA8 format (got {:?})",
                format
            ),
        };
        let normal_texture = match normal_texture.format {
            gltf::image::Format::R8G8B8 => Texture::from_rgb8(
                &normal_texture.pixels,
                normal_texture.width,
                normal_texture.height,
            ),
            gltf::image::Format::R8G8B8A8 => Texture::from_rgba8(
                &normal_texture.pixels,
                normal_texture.width,
                normal_texture.height,
            ),
            format => bail!(
                "Normal texture must be in RGB8 or RGBA8 format (got {:?})",
                format
            ),
        };
        let metallic_roughness_texture = match metallic_roughness_texture.format {
            gltf::image::Format::R8G8B8 => Texture::from_rgb8(
                &metallic_roughness_texture.pixels,
                metallic_roughness_texture.width,
                metallic_roughness_texture.height,
            ),
            gltf::image::Format::R8G8B8A8 => Texture::from_rgba8(
                &metallic_roughness_texture.pixels,
                metallic_roughness_texture.width,
                metallic_roughness_texture.height,
            ),
            format => bail!(
                "Metallic/Roughness texture must be in RGB8 or RGBA8 format (got {:?})",
                format
            ),
        };
        let ao_texture = match ao_texture.format {
            gltf::image::Format::R8G8B8 => {
                Texture::from_rgb8(&ao_texture.pixels, ao_texture.width, ao_texture.height)
            }
            gltf::image::Format::R8G8B8A8 => {
                Texture::from_rgba8(&ao_texture.pixels, ao_texture.width, ao_texture.height)
            }
            format => bail!(
                "Ambient Occlusion texture must be in RGB8 or RGBA8 format (got {:?})",
                format
            ),
        };

        let material = Material {
            diffuse: diffuse.into(),
            diffuse_texture: textures.insert(diffuse_texture),
            normal_texture: textures.insert(normal_texture),
            metallic,
            roughness,
            metallic_roughness_texture: textures.insert(metallic_roughness_texture),
            ao,
            ao_texture: textures.insert(ao_texture),
            texture_scale: 1.0,
        };

        Ok(material)
    }
}

#[derive(Debug, Copy, Clone, ShaderType)]
#[repr(C)]
pub struct MaterialMetaUniform {
    diffuse: Color,
    metallic: f32,
    roughness: f32,
    ao: f32,
    texture_scale: f32,
}

#[derive(Asset)]
pub struct GpuMaterial {
    pub meta: GpuBufferVec<MaterialMetaUniform>,

    pub diffuse_texture: GpuTexture,
    pub diffuse_texture_sampler: wgpu::Sampler,
    pub normal_texture: GpuTexture,
    pub normal_texture_sampler: wgpu::Sampler,
    pub metallic_roughness_texture: GpuTexture,
    pub metallic_roughness_texture_sampler: wgpu::Sampler,
    pub ao_texture: GpuTexture,
    pub ao_texture_sampler: wgpu::Sampler,
}

impl RenderAsset for GpuMaterial {
    type Source = Material;
    type Param = Extract<'static, 'static, Res<'static, Assets<Texture>>>;

    fn extract_render_asset(
        base_asset: &Material,
        textures: &Extract<Res<Assets<Texture>>>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let diffuse_texture = textures.get(base_asset.diffuse_texture)?;
        let diffuse_texture =
            GpuTexture::from_image(device, queue, &diffuse_texture, texture_format::SDR_FORMAT)?;

        let normal_texture = textures.get(base_asset.normal_texture)?;
        let normal_texture = GpuTexture::from_image(
            device,
            queue,
            &normal_texture,
            texture_format::NORMAL_FORMAT,
        )?;

        let metallic_roughness_texture = textures.get(base_asset.metallic_roughness_texture)?;
        let metallic_roughness_texture = GpuTexture::from_image(
            device,
            queue,
            &metallic_roughness_texture,
            texture_format::SDR_FORMAT,
        )?;

        let ao_texture = textures.get(base_asset.ao_texture)?;
        let ao_texture =
            GpuTexture::from_image(device, queue, &ao_texture, texture_format::SDR_FORMAT)?;

        let mut meta =
            GpuBufferVec::new(wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);

        let diffuse_texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Diffuse Texture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let normal_texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Normal Texture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let metallic_roughness_texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Metallic Roughness Texture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let ao_texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("AO Texture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let meta_uniform = MaterialMetaUniform {
            diffuse: base_asset.diffuse,
            metallic: base_asset.metallic,
            roughness: base_asset.roughness,
            ao: base_asset.ao,
            texture_scale: base_asset.texture_scale,
        };
        meta.push(meta_uniform);
        meta.enqueue_update(device, queue);

        Some(Self {
            meta,
            diffuse_texture,
            diffuse_texture_sampler,
            normal_texture,
            normal_texture_sampler,
            metallic_roughness_texture,
            metallic_roughness_texture_sampler,
            ao_texture,
            ao_texture_sampler,
        })
    }

    fn update_render_asset(
        &mut self,
        base_asset: &Self::Source,
        _textures: &Extract<Res<Assets<Texture>>>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()>
    where
        Self: Sized,
    {
        let meta = MaterialMetaUniform {
            diffuse: base_asset.diffuse,
            metallic: base_asset.metallic,
            roughness: base_asset.roughness,
            ao: base_asset.ao,
            texture_scale: base_asset.texture_scale,
        };

        self.meta.clear();
        self.meta.push(meta);

        self.meta.enqueue_update(device, queue);

        Ok(())
    }
}

impl CreateBindGroup for GpuMaterial {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Material Bind Group Layout"),
            entries: &[
                // Material meta buffer
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
                // Diffuse texture sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Normal texture
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
                // Normal texture sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Metallic roughness texture
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Metallic roughness texture sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // AO texture
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // AO texture sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        cached_layout: &BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout: cached_layout,
            entries: &[
                // Material meta buffer
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.meta.binding().unwrap(),
                },
                // Diffuse texture
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.diffuse_texture.view),
                },
                // Diffuse texture sampler
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.diffuse_texture_sampler),
                },
                // Normal texture
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.normal_texture.view),
                },
                // Normal texture sampler
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.normal_texture_sampler),
                },
                // Metallic roughness texture
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(
                        &self.metallic_roughness_texture.view,
                    ),
                },
                // Metallic roughness texture sampler
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(
                        &self.metallic_roughness_texture_sampler,
                    ),
                },
                // AO texture
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&self.ao_texture.view),
                },
                // AO texture sampler
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Sampler(&self.ao_texture_sampler),
                },
            ],
        })
    }
}

pub struct MaterialPlugin;

impl Plugin for MaterialPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_plugin(ExtractRenderAssetPlugin::<GpuMaterial>::default())?;
        render_app.add_plugin(AssetBindGroupPlugin::<GpuMaterial>::default())?;
        Ok(())
    }
}
