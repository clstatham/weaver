#![allow(clippy::too_many_arguments, clippy::from_over_into)]

pub mod aabb;
pub mod app;
pub mod asset_server;
pub mod camera;
pub mod color;
pub mod doodads;
pub mod input;
pub mod light;
pub mod material;
pub mod mesh;
pub mod model;
pub mod particles;
pub mod physics;
pub mod renderer;
pub mod texture;
pub mod time;
pub mod transform;
pub mod ui;

pub mod prelude {
    pub use crate::{
        aabb::Aabb,
        app::App,
        asset_server::AssetServer,
        camera::Camera,
        color::Color,
        doodads::{Cone, Cube, Doodad, Doodads},
        input::{Input, KeyCode, MouseButton},
        light::{DirectionalLight, PointLight},
        material::Material,
        mesh::Mesh,
        model::{ModelBundle, RigidBodyModelBundle},
        particles::ParticleEmitter,
        physics::{RapierContext, RigidBody},
        renderer::Renderer,
        texture::{Texture, TextureFormat},
        time::Time,
        transform::Transform,
        ui::EguiContext,
    };
    pub use weaver_proc_macro::{Bundle, Component};
}
