use std::{path::Path, sync::Arc};

use weaver_ecs::{
    prelude::{Resource, World},
    world::FromWorld,
};
use weaver_renderer::{prelude::wgpu, WgpuDevice, WgpuQueue};
use weaver_util::Result;
use wgpu::util::DeviceExt;

use super::Skybox;

pub const SKYBOX_IRRADIANCE_SIZE: u32 = 64;
pub const SKYBOX_SPECULAR_SIZE: u32 = 128;
pub const SKYBOX_SPECULAR_MIP_LEVELS: u32 = 5;

pub struct LoadedKtx {
    pub header: ktx2::Header,
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
        .flat_map(|data| data.to_vec())
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

#[derive(Clone, Resource)]
pub struct GpuSkyboxIrradiance {
    #[allow(unused)]
    pub diffuse_texture: Arc<wgpu::Texture>,
    pub diffuse_cube_view: Arc<wgpu::TextureView>,
    #[allow(unused)]
    pub specular_texture: Arc<wgpu::Texture>,
    pub specular_cube_view: Arc<wgpu::TextureView>,
    #[allow(unused)]
    pub brdf_lut_texture: Arc<wgpu::Texture>,
    pub brdf_lut_view: Arc<wgpu::TextureView>,

    pub sampler: Arc<wgpu::Sampler>,
}

impl FromWorld for GpuSkyboxIrradiance {
    fn from_world(world: &mut World) -> Self {
        let skybox = world.get_resource::<Skybox>().unwrap();
        let device = world.get_resource::<WgpuDevice>().unwrap().into_inner();
        let queue = world.get_resource::<WgpuQueue>().unwrap().into_inner();

        let diffuse = load_ktx(&skybox.diffuse_path, device, queue).unwrap();
        let specular = load_ktx(&skybox.specular_path, device, queue).unwrap();
        let brdf_lut_texture = load_png(&skybox.brdf_lut_path, device, queue).unwrap();

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("skybox_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let diffuse_cube_view = diffuse.texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("diffuse_cube_view"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let specular_cube_view = specular.texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("specular_cube_view"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let brdf_lut_view = brdf_lut_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("brdf_lut_view"),
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..Default::default()
        });

        Self {
            diffuse_texture: diffuse.texture,
            diffuse_cube_view: Arc::new(diffuse_cube_view),
            specular_texture: specular.texture,
            specular_cube_view: Arc::new(specular_cube_view),
            brdf_lut_texture,
            brdf_lut_view: Arc::new(brdf_lut_view),
            sampler: Arc::new(sampler),
        }
    }
}
