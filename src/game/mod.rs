use clap::Parser;

use crate::{
    app::Window,
    core::{particles::ParticleEmitter, ui::builtin::FpsDisplay},
    prelude::*,
    renderer::pass::Pass,
};

use self::{
    camera::{FollowCameraController, FollowCameraUpdate},
    npc::NpcUpdate,
    player::PlayerUpdate,
};

pub mod camera;
pub mod maps;
pub mod materials;
pub mod npc;
pub mod player;

#[system(WindowUpdate)]
fn window_update(mut window: ResMut<Window>, input: Res<Input>) {
    window.fps_mode = input.mouse_button_pressed(3);
}

#[system(UiUpdate)]
fn ui_update(
    mut ctx: ResMut<EguiContext>,
    mut fps_display: Query<&mut FpsDisplay>,
    mut renderer: ResMut<Renderer>,
) {
    ctx.draw_if_ready(|ctx| {
        for mut fps_display in fps_display.iter() {
            fps_display.run_ui(ctx);
        }

        egui::Window::new("Render Settings").show(ctx, |ui| {
            let mut shadows_enabled = renderer.shadow_pass.is_enabled();
            ui.checkbox(&mut shadows_enabled, "Shadows");
            if shadows_enabled != renderer.shadow_pass.is_enabled() {
                if shadows_enabled {
                    renderer.shadow_pass.enable();
                } else {
                    renderer.shadow_pass.disable();
                }
            }
        });
    });
}

#[system(Setup)]
fn setup(
    commands: Commands,
    mut asset_server: ResMut<AssetServer>,
    mut renderer: ResMut<Renderer>,
) {
    renderer.shadow_pass.disable();

    commands.spawn(FpsDisplay::new())?;

    let ground = maps::GroundBundle {
        transform: Transform::from_scale_rotation_translation(
            Vec3::new(30.0, 1.0, 30.0),
            Quat::IDENTITY,
            Vec3::new(0.0, -1.0, 0.0),
        ),
        mesh: asset_server.load_mesh("meshes/cube.glb")?,
        material: materials::Materials::WoodTile.load(&mut asset_server, 30.0)?,
        ground: maps::Ground,
    };
    commands.spawn(ground)?;

    let light = PointLight::new(Vec3::new(0.0, 5.0, 0.0), Color::WHITE, 20.0);
    commands.spawn(light)?;

    let player = player::Player {
        speed: 7.0,
        rotation_speed: 32.0,
    };
    let player = commands.spawn(player::PlayerBundle {
        player,
        transform: Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        mesh: asset_server.load_mesh("meshes/monkey_flat.glb")?,
        material: materials::Materials::Metal.load(&mut asset_server, 2.0)?,
    })?;

    let camera_controller = FollowCameraController {
        stiffness: 50.0,
        target: player,
        ..Default::default()
    };
    commands.spawn((camera_controller, Camera::default()))?;

    // bunch of npcs in a circle
    let npc = npc::Npc {
        speed: 0.0,
        rotation_speed: 0.0,
    };
    let npc_mesh = asset_server.load_mesh("meshes/monkey_flat.glb")?;
    let npc_material = materials::Materials::Wood.load(&mut asset_server, 2.0)?;
    let npc_count = 200;
    let npc_radius = 5.0;
    for i in 0..npc_count {
        let angle = (i as f32 / npc_count as f32) * std::f32::consts::TAU;
        let x = angle.cos() * npc_radius;
        let z = angle.sin() * npc_radius;

        commands.spawn(npc::NpcBundle {
            npc,
            transform: Transform::from_translation(Vec3::new(x, 1.0, z)),
            mesh: npc_mesh.clone(),
            material: npc_material.clone(),
        })?;
    }
}

#[derive(Debug, clap::Parser)]
struct Args {
    #[arg(short, long, default_value = "1600")]
    pub width: usize,
    #[arg(short, long, default_value = "900")]
    pub height: usize,
}

pub fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut app = App::new(args.width, args.height)?;

    app.add_startup_system(Setup);

    app.add_system(WindowUpdate);
    app.add_system(ParticleUpdate);
    app.add_system(FollowCameraUpdate);
    app.add_system(UiUpdate);
    app.add_system(PlayerUpdate);
    app.add_system(NpcUpdate);

    app.run()
}
