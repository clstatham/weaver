use std::sync::Arc;

use weaver_app::{plugin::Plugin, prelude::App};
use weaver_asset::loader::AssetLoader;
use weaver_core::texture::{Texture, TextureLoader};
use wgpu::util::DeviceExt;

use crate::Renderer;

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
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
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
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        let mut loader = app.get_resource_mut::<AssetLoader>().unwrap();
        loader.add_loader(TextureLoader);
        Ok(())
    }
}
