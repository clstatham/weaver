pub mod app;
pub mod core;
pub mod ecs;
mod game;
pub mod renderer;

pub mod prelude {
    pub use crate::app::{asset_server::AssetServer, commands::Commands, App};
    pub use crate::core::{
        camera::{Camera, FlyCameraController},
        color::Color,
        input::{Input, KeyCode},
        light::{DirectionalLight, PointLight},
        material::Material,
        mesh::Mesh,
        time::Time,
        transform::Transform,
        ui::EguiContext,
    };
    pub use crate::ecs::*;
    pub use crate::renderer::Renderer;
    pub use glam::*;
    pub use weaver_proc_macro::system;
    pub use winit::event::MouseButton;
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    game::run()
}
