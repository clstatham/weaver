#![allow(clippy::too_many_arguments, clippy::from_over_into)]

pub use wgpu;

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
pub mod particles;
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
        doodads::{Cone, Cube, Doodad, Doodads, Line},
        geom::{Aabb, BoundingSphere, Ray, Rect},
        input::{Input, KeyCode, MouseButton},
        light::{DirectionalLight, PointLight},
        material::Material,
        mesh::Mesh,
        particles::ParticleEmitter,
        renderer::Renderer,
        texture::{Skybox, Texture, TextureFormat},
        time::Time,
        transform::{GlobalTransform, Transform},
        ui::EguiContext,
    };
}
