use encase::ShaderType;
use weaver_app::{App, plugin::Plugin};
use weaver_asset::{Handle, prelude::Asset};
use weaver_core::{color::Color, texture::Texture};
use weaver_ecs::{prelude::Res, system::SystemParamItem};
use weaver_renderer::{
    asset::{
        ExtractHandlePlugin, PrepareAssetError, RenderAsset, RenderAssetPlugin, RenderAssets,
        prepare_after,
    },
    bind_group::{
        AsBindGroup, BindGroup, BindGroupLayoutBuilder, BindGroupLayouts, BindingsBuilder,
    },
    buffer::GpuBufferVec,
    prelude::*,
    texture::{GpuImage, GpuTexture},
};
use weaver_util::prelude::*;

pub const WHITE_TEXTURE: Handle<Texture> =
    Handle::from_u128(171952135557955961317447623731106286307);
pub const BLACK_TEXTURE: Handle<Texture> =
    Handle::from_u128(29903794803500143808416926793703205514);
pub const ERROR_TEXTURE: Handle<Texture> =
    Handle::from_u128(288942464416563327199333453807837020723);

#[derive(Asset, Clone)]
pub struct Material {
    pub diffuse: Color,
    pub diffuse_texture: Handle<Texture>,

    pub normal_texture: Handle<Texture>,

    pub metallic: f32,
    pub roughness: f32,
    pub metallic_roughness_texture: Handle<Texture>,

    pub ao: f32,
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
            ao: 1.0,
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
            ao: 1.0,
            ao_texture: WHITE_TEXTURE,
            texture_scale: 1.0,
        }
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

/// A material's GPU resources, bound as group 0 of the PBR pipeline.
pub struct MaterialBindings {
    pub meta: GpuBufferVec<MaterialMetaUniform>,
    pub diffuse_texture: GpuTexture,
    pub normal_texture: GpuTexture,
    pub metallic_roughness_texture: GpuTexture,
    pub ao_texture: GpuTexture,
    /// Shared by all four textures.
    pub sampler: wgpu::Sampler,
}

impl AsBindGroup for MaterialBindings {
    const LABEL: &'static str = "Material";

    fn layout(layout: &mut BindGroupLayoutBuilder) {
        let fragment = wgpu::ShaderStages::FRAGMENT;
        layout.uniform_buffer(fragment);
        // diffuse, normal, metallic/roughness, AO: each a texture followed by its sampler
        for _ in 0..4 {
            layout.texture_2d(fragment).filtering_sampler(fragment);
        }
    }

    fn bindings<'a>(&'a self, bindings: &mut BindingsBuilder<'a>) {
        bindings.buffer_vec(&self.meta);
        for texture in [
            &self.diffuse_texture,
            &self.normal_texture,
            &self.metallic_roughness_texture,
            &self.ao_texture,
        ] {
            bindings.texture(texture).sampler(&self.sampler);
        }
    }
}

pub struct GpuMaterial {
    pub bindings: MaterialBindings,
    pub bind_group: BindGroup<MaterialBindings>,
}

impl MaterialMetaUniform {
    fn new(material: &Material) -> Self {
        Self {
            diffuse: material.diffuse,
            metallic: material.metallic,
            roughness: material.roughness,
            ao: material.ao,
            texture_scale: material.texture_scale,
        }
    }
}

impl RenderAsset for GpuMaterial {
    type Source = Material;
    type Param = (Res<RenderAssets<GpuImage>>, Res<BindGroupLayouts>);

    fn prepare(
        material: Material,
        (images, layouts): &mut SystemParamItem<Self::Param>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Self, PrepareAssetError<Material>> {
        let (Some(diffuse), Some(normal), Some(metallic_roughness), Some(ao)) = (
            images.get(material.diffuse_texture),
            images.get(material.normal_texture),
            images.get(material.metallic_roughness_texture),
            images.get(material.ao_texture),
        ) else {
            return Err(PrepareAssetError::RetryNextFrame(material));
        };

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Material Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        let mut meta =
            GpuBufferVec::new(wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        meta.push(MaterialMetaUniform::new(&material));
        meta.enqueue_update(device, queue);

        let bindings = MaterialBindings {
            meta,
            diffuse_texture: diffuse.srgb.clone(),
            normal_texture: normal.linear.clone(),
            metallic_roughness_texture: metallic_roughness.srgb.clone(),
            ao_texture: ao.srgb.clone(),
            sampler,
        };
        let bind_group =
            BindGroup::new(device, &layouts.get::<MaterialBindings>(device), &bindings);

        Ok(Self {
            bindings,
            bind_group,
        })
    }
}

pub struct MaterialPlugin;

impl Plugin for MaterialPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_plugin(RenderAssetPlugin::<GpuMaterial>::default())?;
        render_app.add_plugin(ExtractHandlePlugin::<Material>::default())?;
        prepare_after::<GpuMaterial, GpuImage>(render_app);
        Ok(())
    }
}
