use clap::Parser;
use rand::{seq::SliceRandom, Rng};

use crate::{
    app::Window,
    core::{
        doodads::{Cube, Doodad, Doodads},
        light::MAX_LIGHTS,
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
    materials::{Banana, BrickWall, Metal, StoneWall, Wood, WoodTile},
    player::PlayerUpdate,
};

pub mod camera;
pub mod maps;
pub mod materials;
pub mod npc;
pub mod player;

#[derive(Resource, Default)]
pub struct State {
    pub lights: Vec<Entity>,
    pub light_intensity: f32,
    pub light_radius: f32,
    pub npcs: Vec<Entity>,

    pub banana_roughness: f32,
    pub banana_metallic: f32,

    pub tile_roughness: f32,
    pub tile_metallic: f32,

    pub wood_roughness: f32,
    pub wood_metallic: f32,

    pub brick_roughness: f32,
    pub brick_metallic: f32,

    pub stone_roughness: f32,
    pub stone_metallic: f32,

    pub metal_roughness: f32,
    pub metal_metallic: f32,
}

#[system(WindowUpdate)]
fn window_update(mut window: ResMut<Window>, input: Res<Input>) {
    window.fps_mode = input.mouse_button_pressed(3);
}

#[system(UiUpdate)]
fn ui_update(
    commands: Commands,
    mut asset_server: ResMut<AssetServer>,
    mut ctx: ResMut<EguiContext>,
    mut fps_display: Query<&mut FpsDisplay>,
    mut renderer: ResMut<Renderer>,
    mut state: ResMut<State>,
    mut point_lights: Query<&mut PointLight>,
) {
    ctx.draw_if_ready(|ctx| {
        for mut fps_display in fps_display.iter() {
            fps_display.run_ui(ctx);
        }

        let mut n_npcs = state.npcs.len();

        let mut shadow_pass_enabled = renderer.shadow_pass.is_enabled();
        let mut n_lights = state.lights.len();
        let mut light_intensity = state.light_intensity;
        let mut light_radius = state.light_radius;

        egui::Window::new("Settings")
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.heading("Renderer");
                ui.checkbox(&mut shadow_pass_enabled, "Shadows");

                ui.heading("Lights");
                ui.add(egui::Slider::new(&mut n_lights, 0..=MAX_LIGHTS).text("Count"));
                ui.add(egui::Slider::new(&mut light_intensity, 0.0..=100.0).text("Intensity"));
                ui.add(egui::Slider::new(&mut light_radius, 0.0..=100.0).text("Radius"));

                ui.heading("NPCs");
                ui.add(egui::Slider::new(&mut n_npcs, 0..=1000).text("Count"));
            });

        if shadow_pass_enabled != renderer.shadow_pass.is_enabled() {
            if shadow_pass_enabled {
                renderer.shadow_pass.enable();
            } else {
                renderer.shadow_pass.disable();
            }
        }

        if n_lights != state.lights.len() {
            for light in state.lights.drain(..) {
                commands.remove_entity(light);
            }
            let light_colors = [
                Color::WHITE,
                Color::RED,
                Color::GREEN,
                Color::BLUE,
                Color::YELLOW,
                Color::CYAN,
                Color::MAGENTA,
            ];

            let light_count = n_lights;
            let mut rng = rand::thread_rng();
            for _i in 0..light_count {
                // let angle = (i as f32 / light_count as f32) * std::f32::consts::TAU;
                let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                let light_radius = rng.gen_range(0.0..30.0);
                let x = angle.cos() * light_radius;
                let z = angle.sin() * light_radius;
                let y = rng.gen_range(0.0..10.0);

                let light = PointLight::new(
                    Vec3::new(x, y, z),
                    light_colors[rng.gen_range(0..light_colors.len())],
                    light_intensity,
                    light_radius,
                );
                state.lights.push(commands.spawn(light).unwrap());
            }
        }

        if light_intensity != state.light_intensity {
            for mut light in point_lights.iter() {
                light.intensity = light_intensity;
            }
            state.light_intensity = light_intensity;
        }

        if light_radius != state.light_radius {
            for mut light in point_lights.iter() {
                light.radius = light_radius;
            }
            state.light_radius = light_radius;
        }

        if n_npcs != state.npcs.len() {
            for npc in state.npcs.drain(..) {
                commands.remove_entity(npc);
            }
            let npc_mesh = asset_server
                .load_mesh("meshes/monkey_2x.glb", &renderer)
                .unwrap();
            let npc_materials = [
                asset_server
                    .load_material("materials/wood_025.glb")
                    .unwrap(),
                asset_server
                    .load_material("materials/metal_006.glb")
                    .unwrap(),
            ];
            let npc_count = n_npcs;
            let mut rng = rand::thread_rng();
            for _ in 0..npc_count {
                // let angle = (i as f32 / npc_count as f32) * std::f32::consts::TAU;
                let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                let npc_radius = rng.gen_range(0.0..30.0);
                let x = angle.cos() * npc_radius;
                let z = angle.sin() * npc_radius;
                let y = rng.gen_range(0.0..50.0);

                let npc = npc::Npc {
                    speed: 0.0,
                    rotation_speed: 0.0,
                };

                let material_index = rng.gen_range(0..npc_materials.len());

                let npc_material = npc_materials[material_index].clone();
                let npc = match material_index {
                    0 => commands.spawn((
                        npc,
                        Transform::from_translation(Vec3::new(x, y, z)),
                        npc_mesh.clone(),
                        npc_material,
                        Wood,
                    )),
                    1 => commands.spawn((
                        npc,
                        Transform::from_translation(Vec3::new(x, y, z)),
                        npc_mesh.clone(),
                        npc_material,
                        Metal,
                    )),
                    _ => unreachable!(),
                };

                state.npcs.push(npc.unwrap());
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
fn debug_lights(mut doodads: ResMut<Doodads>, mut point_lights: Query<&PointLight>) {
    for light in point_lights.iter() {
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
    renderer.shadow_pass.disable();
    renderer.gbuffer_pass.disable();
    renderer.sky_pass.disable();

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

    let mut material = asset_server.load_material("materials/wood_herringbone_tiles_004.glb")?;
    material.texture_scaling = 100.0;

    let ground = maps::GroundBundle {
        transform: Transform::from_scale_rotation_translation(
            Vec3::new(100.0, 1.0, 100.0),
            Quat::IDENTITY,
            Vec3::new(0.0, -1.0, 0.0),
        ),
        mesh: asset_server.load_mesh("meshes/cube.obj", &renderer)?,
        material,
        ground: maps::Ground,
    };
    commands.spawn((ground, WoodTile))?;

    let player = player::Player {
        speed: 14.0,
        rotation_speed: 0.002,
    };
    let player = commands.spawn((
        player::PlayerBundle {
            player,
            transform: Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
            mesh: asset_server.load_mesh("meshes/monkey_2x.glb", &renderer)?,
            material: asset_server.load_material("materials/wood_025.glb")?,
        },
        Banana,
    ))?;

    let camera_controller = FollowCameraController {
        stiffness: 50.0,
        target: player,
        pitch_sensitivity: 0.002,
        ..Default::default()
    };
    commands.spawn((camera_controller, Camera::new()))?;
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

    app.insert_resource(State {
        light_intensity: 10.0,
        light_radius: 30.0,
        ..Default::default()
    })?;

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
