use std::sync::Arc;

use format::TEXTURE_FORMAT;
use weaver_app::{plugin::Plugin, prelude::App};
use weaver_core::texture::Texture;
use wgpu::util::DeviceExt;

use crate::Renderer;

pub mod format {
    pub use wgpu::TextureFormat;
    pub const VIEW_FORMAT: TextureFormat = TextureFormat::Bgra8UnormSrgb;
    pub const TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;
    pub const NORMAL_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb; // todo: change to Rgba8Unorm and remove the gamma correction in the shader
    pub const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;
}

#[derive(Clone)]
pub struct GpuTexture {
    pub texture: Arc<wgpu::Texture>,
    pub view: Arc<wgpu::TextureView>,
}

impl GpuTexture {
    pub fn from_image(renderer: &Renderer, image: &Texture) -> Option<Self> {
        let texture = renderer.device().create_texture_with_data(
            renderer.queue(),
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
                format: TEXTURE_FORMAT,
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
    fn build(&self, _app: &mut App) -> anyhow::Result<()> {
        Ok(())
    }
}
