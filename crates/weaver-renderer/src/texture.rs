use std::sync::Arc;

use weaver_app::{plugin::Plugin, prelude::App};
use weaver_asset::AssetApp;
use weaver_core::texture::Texture;
use weaver_util::Result;
use wgpu::util::DeviceExt;

pub mod texture_format {
    pub use wgpu::TextureFormat;
    pub const VIEW_FORMAT: TextureFormat = TextureFormat::Bgra8UnormSrgb;
    pub const SDR_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;
    pub const HDR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
    pub const HDR_CUBE_FORMAT: TextureFormat = TextureFormat::Rgba32Float;
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

pub struct GpuTextureArray {
    pub backing_texture: Arc<wgpu::Texture>,
    pub backing_view: Arc<wgpu::TextureView>,
}

impl GpuTextureArray {
    pub fn from_images(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        images: &[Texture],
    ) -> Option<Self> {
        let layer_count = images.len() as u32;
        assert!(
            layer_count > 0,
            "Texture array must have at least one layer"
        );
        assert!(
            images
                .iter()
                .all(|image| image.width() == images[0].width()),
            "All images in texture array must have the same width"
        );
        assert!(
            images
                .iter()
                .all(|image| image.height() == images[0].height()),
            "All images in texture array must have the same height"
        );
        let width = images[0].width();
        let height = images[0].height();

        let data = images
            .iter()
            .flat_map(|image| image.to_rgba8())
            .collect::<Vec<u8>>();

        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Texture Array"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: layer_count,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &data,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(layer_count),
            base_array_layer: 0,
            ..Default::default()
        });

        Some(Self {
            backing_texture: Arc::new(texture),
            backing_view: Arc::new(view),
        })
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.backing_texture.format()
    }

    pub fn layer_count(&self) -> u32 {
        self.backing_texture.size().depth_or_array_layers
    }

    pub fn width(&self) -> u32 {
        self.backing_texture.size().width
    }

    pub fn height(&self) -> u32 {
        self.backing_texture.size().height
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.backing_view
    }

    pub fn as_binding_resource(&self) -> wgpu::BindingResource {
        wgpu::BindingResource::TextureView(&self.backing_view)
    }
}

pub struct TexturePlugin;

impl Plugin for TexturePlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_asset::<Texture>();
        Ok(())
    }
}
