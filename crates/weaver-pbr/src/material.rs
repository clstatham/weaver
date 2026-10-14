//! The [`Material`] asset and the render-world resources built from it.

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

/// A 1x1 opaque white texture, the default for texture slots with no image.
pub const WHITE_TEXTURE: Handle<Texture> =
    Handle::from_u128(171952135557955961317447623731106286307);
/// A 1x1 opaque black texture. As a normal map it means "no normal map".
pub const BLACK_TEXTURE: Handle<Texture> =
    Handle::from_u128(29903794803500143808416926793703205514);
/// A 2x2 magenta and black checkerboard, for marking missing textures.
pub const ERROR_TEXTURE: Handle<Texture> =
    Handle::from_u128(288942464416563327199333453807837020723);

/// A surface description for the PBR pipeline.
///
/// The scalar factors multiply what the textures provide: `metallic` and `roughness` scale the
/// blue and green channels of `metallic_roughness_texture` (glTF layout), `ao` scales the red
/// channel of `ao_texture`, and `diffuse` tints `diffuse_texture`. Every texture slot has to be
/// filled; use [`WHITE_TEXTURE`] or [`BLACK_TEXTURE`] for the ones you don't have. A black
/// normal texture means the mesh normal is used as is.
#[derive(Asset, Clone)]
pub struct Material {
    /// Base color factor, multiplied with the diffuse texture.
    pub diffuse: Color,
    /// Albedo texture.
    pub diffuse_texture: Handle<Texture>,

    /// Tangent-space normal map.
    pub normal_texture: Handle<Texture>,

    /// Metallic factor, multiplied with the metallic-roughness texture.
    pub metallic: f32,
    /// Roughness factor, multiplied with the metallic-roughness texture.
    pub roughness: f32,
    /// Metalness in the blue channel, roughness in the green channel.
    pub metallic_roughness_texture: Handle<Texture>,

    /// Ambient occlusion factor, multiplied with the AO texture.
    pub ao: f32,
    /// Ambient occlusion in the red channel.
    pub ao_texture: Handle<Texture>,

    /// Multiplier on the UV coordinates, for tiling all four textures at once.
    pub texture_scale: f32,
}

/// A plain material of the given color: no metal, no normal map, full AO.
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

/// A plain material that's just the given albedo texture.
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

/// The scalar part of a [`Material`], as the uniform buffer the shader reads.
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
    /// One-element uniform buffer holding the scalar factors.
    pub meta: GpuBufferVec<MaterialMetaUniform>,
    /// Albedo.
    pub diffuse_texture: GpuTexture,
    /// Tangent-space normal map.
    pub normal_texture: GpuTexture,
    /// Metalness (blue) and roughness (green).
    pub metallic_roughness_texture: GpuTexture,
    /// Ambient occlusion (red).
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

/// A [`Material`] prepared for rendering: uniform buffer, texture views and the bind group.
///
/// Prepared from the [`Material`] asset once all four of its textures have a `GpuImage`;
/// until then preparation is retried every frame.
pub struct GpuMaterial {
    /// The resources the bind group was built from.
    pub bindings: MaterialBindings,
    /// Bound as group 0 when drawing.
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

// FIXME: only rebuilt on `Material` events, so it keeps stale views if a texture is replaced
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

        // FIXME: metallic-roughness and AO are linear data but are bound through the sRGB views
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

/// Registers [`GpuMaterial`] as a render asset and extracts [`Handle<Material>`] components.
pub struct MaterialPlugin;

impl Plugin for MaterialPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_plugin(RenderAssetPlugin::<GpuMaterial>::default())?;
        render_app.add_plugin(ExtractHandlePlugin::<Material>::default())?;
        prepare_after::<GpuMaterial, GpuImage>(render_app);
        Ok(())
    }
}
