#![allow(clippy::too_many_arguments, clippy::from_over_into)]

pub mod app;
pub mod asset_server;
pub mod camera;
pub mod color;
pub mod doodads;
pub mod geom;
pub mod input;
pub mod light;
pub mod material;
pub mod mesh;
pub mod model;
pub mod particles;
pub mod physics;
pub mod relations;
pub mod renderer;
pub mod scripts;
pub mod texture;
pub mod time;
pub mod transform;
pub mod ui;

pub mod prelude {
    pub use crate::{
        app::App,
        asset_server::AssetServer,
        camera::{Camera, FlyCameraController},
        color::Color,
        doodads::{Cone, Cube, Doodad, Doodads},
        geom::Aabb,
        input::{Input, KeyCode, MouseButton},
        light::{DirectionalLight, PointLight},
        material::Material,
        mesh::Mesh,
        model::{ModelBundle, RigidBodyModelBundle},
        particles::ParticleEmitter,
        physics::{RapierContext, RigidBody},
        renderer::Renderer,
        texture::{Texture, TextureFormat},
        time::{RenderTime, UpdateTime},
        transform::{GlobalTransform, Transform},
        ui::EguiContext,
    };
    pub use fabricate;
    pub use weaver_proc_macro::Bundle;
}
