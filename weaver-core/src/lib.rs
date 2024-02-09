#![allow(clippy::too_many_arguments, clippy::from_over_into)]

pub use wgpu;

pub mod app;
pub mod asset_server;
pub mod camera;
pub mod color;
pub mod doodads;
pub mod ecs_ext;
pub mod geom;
pub mod input;
pub mod light;
pub mod material;
pub mod mesh;
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
        doodads::{Cone, Cube, Doodad, Doodads, Line},
        geom::{Aabb, BoundingSphere, Ray, Rect},
        input::{Input, KeyCode, MouseButton},
        light::{DirectionalLight, PointLight},
        material::Material,
        mesh::Mesh,
        particles::ParticleEmitter,
        physics::{RapierContext, RigidBody},
        renderer::Renderer,
        texture::{Skybox, Texture, TextureFormat},
        time::Time,
        transform::{GlobalTransform, Transform},
        ui::EguiContext,
    };
}

pub(crate) fn register_names() {
    use crate::prelude::*;
    use fabricate::registry::StaticId;

    App::register_static_name("App");
    AssetServer::register_static_name("AssetServer");
    Camera::register_static_name("Camera");
    Color::register_static_name("Color");
    Cone::register_static_name("Cone");
    Cube::register_static_name("Cube");
    Line::register_static_name("Line");
    DirectionalLight::register_static_name("DirectionalLight");
    Doodad::register_static_name("Doodad");
    Doodads::register_static_name("Doodads");
    EguiContext::register_static_name("EguiContext");
    FlyCameraController::register_static_name("FlyCameraController");
    GlobalTransform::register_static_name("GlobalTransform");
    Input::register_static_name("Input");
    KeyCode::register_static_name("KeyCode");
    Material::register_static_name("Material");
    Mesh::register_static_name("Mesh");
    MouseButton::register_static_name("MouseButton");
    ParticleEmitter::register_static_name("ParticleEmitter");
    PointLight::register_static_name("PointLight");
    RapierContext::register_static_name("RapierContext");
    Renderer::register_static_name("Renderer");
    RigidBody::register_static_name("RigidBody");
    Skybox::register_static_name("Skybox");
    Texture::register_static_name("Texture");
    Time::register_static_name("Time");
    Transform::register_static_name("Transform");
}
