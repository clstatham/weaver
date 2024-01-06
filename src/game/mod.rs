use clap::Parser;

use crate::{core::ui::builtin::FpsDisplay, prelude::*};

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

#[system(UiUpdate)]
fn ui_update(mut ctx: ResMut<EguiContext>, mut fps_display: Query<&mut FpsDisplay>) {
    ctx.draw_if_ready(|ctx| {
        for mut fps_display in fps_display.iter() {
            fps_display.run_ui(ctx);
        }
    });
}

#[system(Setup)]
fn setup(commands: Commands, mut asset_server: ResMut<AssetServer>) {
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

    let npc = npc::Npc {
        speed: 5.0,
        rotation_speed: 2.0,
    };
    commands.spawn(npc::NpcBundle {
        npc,
        transform: Transform::from_translation(Vec3::new(0.0, 1.0, 5.0)),
        mesh: asset_server.load_mesh("meshes/monkey_flat.glb")?,
        material: materials::Materials::Wood.load(&mut asset_server, 2.0)?,
    })?;
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

    app.add_system(FollowCameraUpdate);
    app.add_system(UiUpdate);
    app.add_system(PlayerUpdate);
    app.add_system(NpcUpdate);

    app.run()
}
