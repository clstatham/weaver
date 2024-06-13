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

#[derive(Component)]
struct Floor;

#[derive(Component)]
struct Object;

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
        .add_system(camera::update_aspect_ratio, SystemStage::Update)?
        .add_system(update, SystemStage::Update)?
        .add_system(ui, SystemStage::Ui)?
        .run()
}

fn setup(world: &Arc<World>) -> Result<()> {
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

    let mut assets = world.get_resource_mut::<Assets>().unwrap();

    let mesh = assets.load::<Mesh>("assets/meshes/cube.obj")?;

    let material = assets.load::<Material>("assets/materials/metal.glb")?;
    {
        let material = assets.get_mut(material).unwrap();
        material.texture_scale = 100.0;
        material.diffuse = Color::WHITE;
    }

    let material2 = assets.load::<Material>("assets/materials/metal.glb")?;
    {
        let material = assets.get_mut(material2).unwrap();
        material.texture_scale = 20.0;
        material.diffuse = Color::RED;
    }

    let _ground = scene.spawn((
        mesh,
        material,
        Transform {
            translation: Vec3::new(0.0, -1.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(20.0, 1.0, 20.0),
        },
        Floor,
    ));

    // circle of lights
    const COLORS: &[Color] = &[
        Color::RED,
        Color::GREEN,
        Color::BLUE,
        Color::YELLOW,
        Color::CYAN,
        Color::MAGENTA,
    ];

    for (i, color) in COLORS.iter().enumerate() {
        let angle = i as f32 / (COLORS.len() as f32) * std::f32::consts::PI * 2.0;
        let _light: Node = scene.spawn(PointLight {
            color: *color,
            intensity: 100.0,
            radius: 100.0,
            position: Vec3::new(angle.cos() * 10.0, 10.0, angle.sin() * 10.0),
        });
    }

    // spawn some meshes
    for i in 0..6 {
        let angle = i as f32 / 6.0 * std::f32::consts::PI * 2.0;
        let _mesh = scene.spawn((
            mesh,
            material2,
            Transform {
                translation: Vec3::new(angle.cos() * 5.0, 2.0, angle.sin() * 5.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.5),
            },
            Object,
        ));
    }

    Ok(())
}

fn update(world: &Arc<World>) -> Result<()> {
    let time = world.get_resource::<Time>().unwrap();
    let query = world.query_filtered::<&mut Transform, With<Object>>();
    for (_entity, mut transform) in query.iter() {
        let angle = time.total_time * 0.5;
        transform.rotation = Quat::from_rotation_y(angle);
    }

    Ok(())
}

fn ui(egui_context: Res<EguiContext>) -> Result<()> {
    egui_context.draw_if_ready(|ctx| {
        egui::Window::new("Hello World").show(ctx, |ui| {
            ui.label("Hello World!");
        });
    });
    Ok(())
}
