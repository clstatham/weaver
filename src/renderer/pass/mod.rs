use crate::{ecs::World, renderer::Renderer};

pub mod doodads;
pub mod hdr;
pub mod particles;
pub mod pbr;
pub mod shadow;
pub mod sky;

pub trait Pass: Send + Sync + 'static {
    fn is_enabled(&self) -> bool;
    fn enable(&mut self);
    fn disable(&mut self);

    fn prepare(&self, world: &World, renderer: &Renderer) -> anyhow::Result<()>;

    fn prepare_if_enabled(&self, world: &World, renderer: &Renderer) -> anyhow::Result<()> {
        if self.is_enabled() {
            self.prepare(world, renderer)?;
        }

        Ok(())
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_target: &wgpu::TextureView,
        depth_target: &wgpu::TextureView,
        renderer: &Renderer,
        world: &World,
    ) -> anyhow::Result<()>;

    fn render_if_enabled(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_target: &wgpu::TextureView,
        depth_target: &wgpu::TextureView,
        renderer: &Renderer,
        world: &World,
    ) -> anyhow::Result<()> {
        if self.is_enabled() {
            self.render(encoder, color_target, depth_target, renderer, world)?;
        }

        Ok(())
    }
}
