use weaver_ecs::world::World;

use crate::{
    geom::Rect,
    texture::{DepthTexture, SdrTexture, TextureFormat},
};

use super::{
    internals::{BindGroupLayoutCache, GpuResourceManager},
    pass::{hdr::HdrRenderPass, Pass},
    Renderer,
};

pub struct Viewport {
    pub enabled: bool,

    pub rect: Rect,

    pub hdr_pass: HdrRenderPass,
    pub color_texture: SdrTexture,
    pub depth_texture: DepthTexture,
}

impl Viewport {
    pub fn new(rect: Rect, device: &wgpu::Device, cache: &BindGroupLayoutCache) -> Self {
        let hdr_pass = HdrRenderPass::new(device, rect.width as u32, rect.height as u32, cache);
        let color_texture = SdrTexture::new(
            rect.width as u32,
            rect.height as u32,
            Some("Viewport Color Texture"),
        );
        let depth_texture = DepthTexture::new(
            rect.width as u32,
            rect.height as u32,
            Some("Viewport Depth Texture"),
        );

        Self {
            enabled: true,
            rect,
            hdr_pass,
            color_texture,
            depth_texture,
        }
    }

    pub fn resize(&mut self, renderer: &Renderer, width: u32, height: u32) {
        self.rect.width = width as f32;
        self.rect.height = height as f32;

        self.hdr_pass.resize(renderer, width, height);
        self.color_texture.resize(renderer, width, height);
        self.depth_texture.resize(renderer, width, height);
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        self.rect.x = x;
        self.rect.y = y;
    }

    pub fn set_rect(&mut self, renderer: &Renderer, rect: Rect) {
        self.move_to(rect.x, rect.y);
        self.resize(renderer, rect.width as u32, rect.height as u32);
    }

    pub fn color_view(&self, manager: &GpuResourceManager) -> wgpu::TextureView {
        self.color_texture
            .handle()
            .lazy_init(manager)
            .unwrap()
            .get_texture()
            .unwrap()
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Viewport Color View"),
                format: Some(SdrTexture::FORMAT),
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::All,
                ..Default::default()
            })
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        renderer: &Renderer,
        world: &World,
    ) -> anyhow::Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let color_texture = self
            .color_texture
            .handle()
            .lazy_init(&renderer.resource_manager)?;
        let depth_texture = self
            .depth_texture
            .handle()
            .lazy_init(&renderer.resource_manager)?;

        let color_view = color_texture
            .get_texture()
            .unwrap()
            .create_view(&Default::default());

        let depth_view = depth_texture
            .get_texture()
            .unwrap()
            .create_view(&Default::default());

        self.hdr_pass
            .render(encoder, &color_view, &depth_view, renderer, world)?;

        Ok(())
    }
}
