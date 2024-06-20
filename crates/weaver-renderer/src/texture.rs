use std::sync::Arc;

use weaver_app::{plugin::Plugin, prelude::App};
use weaver_core::texture::Texture;
use weaver_util::prelude::Result;
use wgpu::util::DeviceExt;

pub mod texture_format {
    pub use wgpu::TextureFormat;
    pub const VIEW_FORMAT: TextureFormat = TextureFormat::Bgra8UnormSrgb;
    pub const SDR_FORMAT: TextureFormat = TextureFormat::Bgra8UnormSrgb;
    pub const HDR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
    pub const NORMAL_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;
    pub const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;
}

#[derive(Clone)]
pub struct GpuTexture {
    pub texture: Arc<wgpu::Texture>,
    pub view: Arc<wgpu::TextureView>,
}

impl GpuTexture {
    pub fn new(
        device: &wgpu::Device,
        label: Option<&str>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture: Arc::new(texture),
            view: Arc::new(view),
        }
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &Texture,
        format: wgpu::TextureFormat,
    ) -> Option<Self> {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Texture"),
                size: wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &image.to_rgba8(),
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Some(Self {
            texture: Arc::new(texture),
            view: Arc::new(view),
        })
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.texture.format()
    }
}

pub struct TexturePlugin;

impl Plugin for TexturePlugin {
    fn build(&self, _app: &mut App) -> Result<()> {
        Ok(())
    }
}
