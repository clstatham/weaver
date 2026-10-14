//! Image-based lighting maps: the pre-generated diffuse and specular cubemaps and BRDF lookup
//! table that sit next to a [`Skybox`]'s `.hdr` file.

use std::{path::Path, sync::Arc};

use weaver_renderer::{prelude::wgpu, texture::GpuTexture};
use weaver_util::prelude::*;
use wgpu::util::DeviceExt;

use super::Skybox;

/// Face size in pixels of the diffuse irradiance cubemap the asset generator script produces.
pub const SKYBOX_IRRADIANCE_SIZE: u32 = 64;
/// Face size in pixels of the specular cubemap's base mip level.
pub const SKYBOX_SPECULAR_SIZE: u32 = 128;
/// Number of mip levels in the specular cubemap (one per roughness step).
pub const SKYBOX_SPECULAR_MIP_LEVELS: u32 = 5;

/// A cubemap read from a `.ktx2` file.
pub struct LoadedKtx {
    /// The file's KTX2 header.
    pub header: ktx2::Header,
    /// The uploaded texture: six layers of `Rgba16Float`, all mips of the file.
    pub texture: Arc<wgpu::Texture>,
}

fn load_ktx(
    path: impl AsRef<Path>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<LoadedKtx> {
    let bytes = std::fs::read(path)?;
    let reader = ktx2::Reader::new(&bytes[..])?;
    let header = reader.header();
    let data = reader
        .levels()
        .flat_map(|data| data.data.to_vec())
        .collect::<Vec<_>>();

    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("KTX Texture"),
            size: wgpu::Extent3d {
                width: header.pixel_width,
                height: header.pixel_height,
                depth_or_array_layers: 6,
            },
            mip_level_count: header.level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::MipMajor,
        &data,
    );

    Ok(LoadedKtx {
        header,
        texture: Arc::new(texture),
    })
}

fn load_png(
    path: impl AsRef<Path>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<Arc<wgpu::Texture>> {
    let img = image::open(path)?;
    let img = img.to_rgba8();
    let (width, height) = img.dimensions();
    let data = img.into_raw();

    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("png_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::MipMajor,
        &data,
    );

    Ok(Arc::new(texture))
}

/// Image-based lighting maps: diffuse irradiance and prefiltered specular cube maps, plus the BRDF
/// lookup table.
#[derive(Clone)]
pub struct GpuSkyboxIrradiance {
    /// Diffuse irradiance cubemap.
    pub diffuse: GpuTexture,
    /// Prefiltered specular cubemap, with a mip level per roughness step.
    pub specular: GpuTexture,
    /// Split-sum BRDF lookup table.
    pub brdf_lut: GpuTexture,
    /// Clamping, trilinear sampler used for all three maps.
    pub sampler: Arc<wgpu::Sampler>,
}

impl GpuSkyboxIrradiance {
    /// Loads the maps that belong to `skybox` (see [`Skybox::new`]).
    ///
    /// The `.ktx2` files are assumed to hold uncompressed `Rgba16Float` cubemaps; the format in
    /// the header is not checked.
    ///
    /// # Errors
    ///
    /// Returns an error if any of the three files can't be read or decoded.
    pub fn load(skybox: &Skybox, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self> {
        let diffuse = load_ktx(&skybox.diffuse_path, device, queue)?;
        let specular = load_ktx(&skybox.specular_path, device, queue)?;
        let brdf_lut = load_png(&skybox.brdf_lut_path, device, queue)?;

        let cube_view = |texture: &wgpu::Texture, label| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(label),
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            })
        };
        let diffuse_view = cube_view(&diffuse.texture, "diffuse_cube_view");
        let specular_view = cube_view(&specular.texture, "specular_cube_view");
        let brdf_lut_view = brdf_lut.create_view(&wgpu::TextureViewDescriptor {
            label: Some("brdf_lut_view"),
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..Default::default()
        });

        Ok(Self {
            diffuse: GpuTexture::from_parts(arc_into_inner(diffuse.texture), diffuse_view),
            specular: GpuTexture::from_parts(arc_into_inner(specular.texture), specular_view),
            brdf_lut: GpuTexture::from_parts(arc_into_inner(brdf_lut), brdf_lut_view),
            sampler: Arc::new(create_sampler(device)),
        })
    }

    /// Black maps, for scenes without a skybox: image-based lighting contributes nothing, and only
    /// the scene's lights light it.
    pub fn fallback(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let black_cube = || {
            let texture = device.create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("Fallback Environment Cube"),
                    size: wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 6,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &[0; 8 * 6],
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            });
            GpuTexture::from_parts(texture, view)
        };
        let brdf_lut = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Fallback BRDF LUT"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[0; 4],
        );
        let brdf_lut_view = brdf_lut.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            diffuse: black_cube(),
            specular: black_cube(),
            brdf_lut: GpuTexture::from_parts(brdf_lut, brdf_lut_view),
            sampler: Arc::new(create_sampler(device)),
        }
    }
}

fn create_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("skybox_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    })
}

fn arc_into_inner(texture: Arc<wgpu::Texture>) -> wgpu::Texture {
    Arc::into_inner(texture).expect("freshly loaded texture is not shared")
}
