use clap::Parser;

use crate::{core::ui::builtin::FpsUi, prelude::*};

use self::camera::{FollowCamera, FollowCameraUpdate};

pub mod camera;
pub mod maps;
pub mod materials;
pub mod player;

#[system(UiUpdate)]
fn ui_update(mut ctx: ResMut<EguiContext>, mut fps_ui: Query<&mut FpsUi>) {
    ctx.draw_if_ready(|ctx| {
        for mut fps_ui in fps_ui.iter() {
            fps_ui.run_ui(ctx);
        }
    });
}

#[system(Setup)]
fn setup(
    commands: Commands,
    mut asset_server: ResMut<AssetServer>,
    mut camera: ResMut<FollowCamera>,
) {
    commands.spawn(FpsUi::new())?;

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

    let light = PointLight::new(Vec3::new(0.0, 5.0, 0.0), Color::WHITE, 100.0);
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
    camera.target = player;
}

#[derive(Debug, clap::Parser)]
struct Args {
    #[clap(short, long, default_value = "1600")]
    pub width: usize,
    #[clap(short, long, default_value = "900")]
    pub height: usize,
}

pub fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut app = App::new(args.width, args.height)?;

    let camera = camera::FollowCamera {
        aspect: args.width as f32 / args.height as f32,
        stiffness: 50.0,
        ..Default::default()
    };
    app.insert_resource(camera)?;

    app.add_startup_system(Setup);

    app.add_system(FollowCameraUpdate);
    app.add_system(UiUpdate);
    app.add_system(player::PlayerUpdate);

    app.run()
}
