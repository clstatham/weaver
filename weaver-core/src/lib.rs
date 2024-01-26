#![allow(clippy::too_many_arguments, clippy::from_over_into)]

use std::sync::Arc;

use weaver_ecs::{prelude::*, registry::Registry};

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
pub mod relations;
pub mod renderer;
pub mod scripts;
pub mod texture;
pub mod time;
pub mod transform;
pub mod ui;

pub mod prelude {
    pub use crate::{
        aabb::Aabb,
        app::App,
        asset_server::AssetServer,
        camera::{Camera, FlyCameraController},
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
        transform::{GlobalTransform, Transform},
        ui::EguiContext,
    };
    pub use weaver_ecs;
    pub use weaver_proc_macro::{Bundle, Component};
}

pub(crate) fn register_all(registry: &Arc<Registry>) {
    use crate::prelude::*;
    registry.get_static::<AssetServer>();
    registry.get_static::<Camera>();
    registry.get_static::<Color>();
    registry.get_static::<Cone>();
    registry.get_static::<Cube>();
    registry.get_static::<Doodads>();
    registry.get_static::<DirectionalLight>();
    registry.get_static::<EguiContext>();
    registry.get_static::<FlyCameraController>();
    registry.get_static::<GlobalTransform>();
    registry.get_static::<Input>();
    registry.get_static::<Material>();
    registry.get_static::<Mesh>();
    registry.get_static::<ParticleEmitter>();
    registry.get_static::<PointLight>();
    registry.get_static::<RapierContext>();
    registry.get_static::<EntityGraph>();
    registry.get_static::<RigidBody>();
    registry.get_static::<Renderer>();
    registry.get_static::<Texture>();
    registry.get_static::<Time>();
    registry.get_static::<Transform>();

    AssetServer::register_vtable(registry);
    Camera::register_vtable(registry);
    Color::register_vtable(registry);
    Cone::register_vtable(registry);
    Cube::register_vtable(registry);
    Doodads::register_vtable(registry);
    DirectionalLight::register_vtable(registry);
    EguiContext::register_vtable(registry);
    FlyCameraController::register_vtable(registry);
    GlobalTransform::register_vtable(registry);
    Input::register_vtable(registry);
    Material::register_vtable(registry);
    Mesh::register_vtable(registry);
    ParticleEmitter::register_vtable(registry);
    PointLight::register_vtable(registry);
    RapierContext::register_vtable(registry);
    EntityGraph::register_vtable(registry);
    RigidBody::register_vtable(registry);
    Renderer::register_vtable(registry);
    Texture::register_vtable(registry);
    Time::register_vtable(registry);
    Transform::register_vtable(registry);
}
