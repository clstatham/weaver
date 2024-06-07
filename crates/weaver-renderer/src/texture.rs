use weaver_app::{plugin::Plugin, prelude::App};
use weaver_asset::{Assets, Handle};
use weaver_core::texture::Texture;
use weaver_ecs::{
    prelude::{Entity, World},
    query::Query,
};
use wgpu::util::DeviceExt;

use crate::{
    extract::{ExtractRenderComponentPlugin, RenderComponent},
    Renderer,
};

pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
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

        Some(Self { texture, view })
    }
}

impl RenderComponent for GpuTexture {
    fn query() -> Query {
        Query::new().read::<Handle<Texture>>()
    }

    fn extract_render_component(entity: Entity, world: &World) -> Option<Self>
    where
        Self: Sized,
    {
        let renderer = world.get_resource::<Renderer>()?;
        let assets = world.get_resource::<Assets>()?;
        let image = world.get_component::<Handle<Texture>>(entity)?;

        let image = assets.get(*image)?;
        Self::from_image(&renderer, image)
    }
}

pub struct TexturePlugin;

impl Plugin for TexturePlugin {
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        app.add_plugin(ExtractRenderComponentPlugin::<GpuTexture>::default())?;
        Ok(())
    }
}
