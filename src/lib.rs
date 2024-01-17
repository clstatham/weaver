pub mod prelude {
    pub use anyhow;
    pub use egui;
    pub use glam::*;
    pub use parking_lot;
    pub use weaver_core::{
        self as weaver_core,
        app::{App, Window},
        asset_server::AssetServer,
        camera::{Camera, FlyCameraController},
        color::Color,
        doodads::*,
        input::{Input, KeyCode},
        light::{DirectionalLight, PointLight},
        material::Material,
        mesh::Mesh,
        particles::{ParticleEmitter, ParticleUpdate},
        renderer::{compute::hdr_loader::HdrLoader, pass::Pass, Renderer},
        time::Time,
        transform::Transform,
        ui::{builtin::*, EguiContext},
    };
    pub use weaver_ecs::{self, *};
    pub use winit::event::MouseButton;
}
