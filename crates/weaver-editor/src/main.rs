use std::sync::Arc;

use weaver::{
    prelude::*,
    weaver_app::{system::SystemStage, App},
    weaver_core::{input::InputPlugin, mesh::Mesh, time::TimePlugin},
    weaver_ecs::world::World,
    weaver_pbr::{camera::PbrCamera, material::Material, PbrPlugin},
    weaver_renderer::{camera::Camera, RendererPlugin},
    weaver_winit::WinitPlugin,
};
use weaver_core::CoreTypesPlugin;
use weaver_diagnostics::frame_time::LogFrameTimePlugin;
use weaver_egui::prelude::*;

pub mod camera;

fn main() -> Result<()> {
    env_logger::init();
    App::new()?
        .add_plugin(CoreTypesPlugin)?
        .add_plugin(WinitPlugin {
            initial_size: (1280, 720),
        })?
        .add_plugin(TimePlugin)?
        .add_plugin(InputPlugin)?
        .add_plugin(AssetPlugin)?
        .add_plugin(RendererPlugin)?
        .add_plugin(PbrPlugin)?
        .add_plugin(EguiPlugin)?
        .add_plugin(LogFrameTimePlugin {
            log_interval: std::time::Duration::from_secs(1),
        })?
        .add_system(setup, SystemStage::Init)?
        .add_system(camera::update_camera, SystemStage::Update)?
        .add_system(ui, SystemStage::Ui)?
        .run()
}

fn setup(world: Arc<World>) -> Result<()> {
    let scene = world.root_scene();
    let _camera = scene.spawn((
        Camera::perspective_lookat(
            Vec3::new(10.0, 10.0, 10.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0f32.to_radians(),
            1280.0 / 720.0,
            0.1,
            100.0,
        ),
        PbrCamera::new(Color::new(0.1, 0.1, 0.1, 1.0)),
        *camera::FlyCameraController {
            aspect: 1280.0 / 720.0,
            ..Default::default()
        }
        .look_at(Vec3::new(10.0, 10.0, 10.0), Vec3::ZERO, Vec3::Y),
    ));

    let asset_loader = world.get_resource::<AssetLoader>().unwrap();

    let mesh = asset_loader.load::<Mesh>("assets/meshes/cube.obj")?;

    let material = asset_loader.load::<Material>("assets/materials/metal.glb")?;
    {
        let mut assets = world.get_resource_mut::<Assets>().unwrap();
        assets.get_mut::<Material>(material).unwrap().texture_scale = 20.0;
    }

    let _ground = scene.spawn((
        mesh,
        material,
        Transform {
            translation: Vec3::new(0.0, -1.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(20.0, 1.0, 20.0),
        },
    ));

    let _light = scene.spawn(PointLight {
        color: Color::WHITE,
        intensity: 100.0,
        radius: 100.0,
        position: Vec3::new(0.0, 10.0, 0.0),
    });

    Ok(())
}

fn ui(_egui_context: Res<EguiContext>) -> Result<()> {
    Ok(())
}
