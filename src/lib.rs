pub mod prelude {
    pub use weaver_core::{
        camera::{Camera, FlyCameraController},
        color::Color,
        input::{Input, KeyCode},
        light::{DirectionalLight, PointLight},
        material::Material,
        mesh::Mesh,
        particles::{ParticleEmitter, ParticleUpdate},
        time::Time,
        transform::Transform,
        ui::EguiContext,
    };
    pub use weaver_ecs::*;
}
