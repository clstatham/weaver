#![allow(clippy::too_many_arguments, clippy::from_over_into)]

use fabricate::registry::StaticId;

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
        time::Time,
        transform::{GlobalTransform, Transform},
        ui::EguiContext,
    };
    pub use fabricate;
    pub use weaver_proc_macro::Bundle;
}

pub(crate) fn register_names() {
    use crate::prelude::*;

    App::register_static_name("App");
    AssetServer::register_static_name("AssetServer");
    Camera::register_static_name("Camera");
    Color::register_static_name("Color");
    Cone::register_static_name("Cone");
    Cube::register_static_name("Cube");
    Doodad::register_static_name("Doodad");
    Doodads::register_static_name("Doodads");
    DirectionalLight::register_static_name("DirectionalLight");
    FlyCameraController::register_static_name("FlyCameraController");
    Input::register_static_name("Input");
    KeyCode::register_static_name("KeyCode");
    MouseButton::register_static_name("MouseButton");
    Material::register_static_name("Material");
    Mesh::register_static_name("Mesh");
    ModelBundle::register_static_name("ModelBundle");
    ParticleEmitter::register_static_name("ParticleEmitter");
    PointLight::register_static_name("PointLight");
    RapierContext::register_static_name("RapierContext");
    RigidBody::register_static_name("RigidBody");
    RigidBodyModelBundle::register_static_name("RigidBodyModelBundle");
    Renderer::register_static_name("Renderer");
    Texture::register_static_name("Texture");
    Transform::register_static_name("Transform");
    Time::register_static_name("Time");
    GlobalTransform::register_static_name("GlobalTransform");
    EguiContext::register_static_name("EguiContext");
}
