use fabricate::prelude::*;

use crate::{
    geom::Rect,
    texture::{DepthTexture, WindowTexture},
};

use super::{
    internals::BindGroupLayoutCache,
    pass::{hdr::HdrRenderPass, Pass},
    Renderer,
};

pub struct Viewport {
    pub enabled: bool,

    pub rect: Rect,

    pub hdr_pass: HdrRenderPass,
    pub color_texture: WindowTexture,
    pub depth_texture: DepthTexture,
}

impl Viewport {
    pub fn new(rect: Rect, device: &wgpu::Device, cache: &BindGroupLayoutCache) -> Self {
        let hdr_pass = HdrRenderPass::new(device, rect.width as u32, rect.height as u32, cache);
        let color_texture = WindowTexture::new(
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

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        renderer: &Renderer,
        output: &wgpu::Texture,
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

        encoder.copy_texture_to_texture(
            color_texture.get_texture().unwrap().as_image_copy(),
            wgpu::ImageCopyTextureBase {
                texture: output,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: self.rect.x as u32,
                    y: self.rect.y as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: (self.rect.width as u32).min(output.size().width - self.rect.x as u32),
                height: (self.rect.height as u32).min(output.size().height - self.rect.y as u32),
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }
}
