use crate::ecs::world::World;

pub mod gpu;
#[macro_use]
pub mod software;

pub trait Renderer {
    fn create(window: &winit::window::Window) -> anyhow::Result<Self>
    where
        Self: Sized;
    fn render(&mut self, world: &mut World) -> anyhow::Result<()>;
}
