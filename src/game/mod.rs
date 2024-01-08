use clap::Parser;

use crate::{
    app::Window,
    core::{
        doodads::{Cube, Doodad, Doodads},
        texture::Skybox,
        ui::builtin::FpsDisplay,
    },
    prelude::*,
    renderer::{
        compute::hdr_loader::HdrLoader,
        pass::{sky::SKYBOX_CUBEMAP_SIZE, Pass},
    },
};

use self::{
    camera::{FollowCameraController, FollowCameraUpdate},
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

        let mut shadow_pass_enabled = renderer.shadow_pass.is_enabled();
        egui::Window::new("Render Settings")
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.checkbox(&mut shadow_pass_enabled, "Shadows");
            });
        if shadow_pass_enabled != renderer.shadow_pass.is_enabled() {
            if shadow_pass_enabled {
                renderer.shadow_pass.enable();
            } else {
                renderer.shadow_pass.disable();
            }
        }
    });
}

#[system(SpinNpcs)]
fn spin_npcs(time: Res<Time>, mut query: Query<&mut Transform, With<npc::Npc>>) {
    for mut transform in query.iter() {
        let mut rotation = transform.get_rotation();
        rotation *= Quat::from_rotation_y(time.delta_seconds * rand::random::<f32>());
        transform.set_rotation(rotation);
    }
}

#[system(DebugLights)]
fn debug_lights(mut doodads: ResMut<Doodads>, mut point_lights: Query<&mut PointLight>) {
    for mut light in point_lights.iter() {
        doodads.push(Doodad::Cube(Cube::new(
            light.position,
            Quat::IDENTITY,
            Vec3::ONE * 0.3,
            light.color,
        )));
    }
}

#[system(Setup)]
fn setup(
    commands: Commands,
    mut asset_server: ResMut<AssetServer>,
    mut renderer: ResMut<Renderer>,
    hdr_loader: Res<HdrLoader>,
) {
    // renderer.shadow_pass.disable();

    commands.spawn(FpsDisplay::new())?;

    let skybox = Skybox {
        texture: asset_server.load_hdr_cubemap(
            "meadow_2k.hdr",
            SKYBOX_CUBEMAP_SIZE,
            &renderer,
            &hdr_loader,
        )?,
    };
    commands.spawn(skybox)?;

    let ground = maps::GroundBundle {
        transform: Transform::from_scale_rotation_translation(
            Vec3::new(30.0, 1.0, 30.0),
            Quat::IDENTITY,
            Vec3::new(0.0, -1.0, 0.0),
        ),
        mesh: asset_server.load_mesh("meshes/cube.glb", &renderer)?,
        material: materials::Materials::WoodTile.load(&mut asset_server, 30.0)?,
        ground: maps::Ground,
    };
    commands.spawn(ground)?;

    let light_colors = [
        Color::WHITE,
        Color::RED,
        Color::GREEN,
        Color::BLUE,
        Color::YELLOW,
        Color::CYAN,
        Color::MAGENTA,
    ];

    let light_count = 7;
    let light_radius = 10.0;
    for i in 0..light_count {
        let angle = (i as f32 / light_count as f32) * std::f32::consts::TAU;
        let x = angle.cos() * light_radius;
        let z = angle.sin() * light_radius;

        let light = PointLight::new(
            Vec3::new(x, 5.0, z),
            light_colors[i % light_colors.len()],
            10.0,
        );
        commands.spawn(light)?;
    }

    let player = player::Player {
        speed: 7.0,
        rotation_speed: 32.0,
    };
    let player = commands.spawn(player::PlayerBundle {
        player,
        transform: Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        mesh: asset_server.load_mesh("meshes/monkey_flat.glb", &renderer)?,
        material: materials::Materials::Metal.load(&mut asset_server, 2.0)?,
    })?;

    let camera_controller = FollowCameraController {
        stiffness: 50.0,
        target: player,
        ..Default::default()
    };
    commands.spawn((camera_controller, Camera::new()))?;

    // bunch of npcs in a circle
    let npc = npc::Npc {
        speed: 0.0,
        rotation_speed: 0.0,
    };
    let npc_mesh = asset_server.load_mesh("meshes/monkey_flat.glb", &renderer)?;
    let npc_material = materials::Materials::Wood.load(&mut asset_server, 2.0)?;
    let npc_count = 20;
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
    #[arg(long, default_value = "1600")]
    pub width: usize,
    #[arg(long, default_value = "900")]
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
    // app.add_system(NpcUpdate);
    app.add_system(SpinNpcs);
    app.add_system(DebugLights);

    app.run()
}
