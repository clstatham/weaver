use std::path::PathBuf;

use clap::Parser;
use rand::Rng;
use rayon::iter::ParallelIterator;

use crate::{
    app::Window,
    core::{
        doodads::{Cube, Doodad, Doodads},
        light::MAX_LIGHTS,
        mesh::MAX_MESHES,
        texture::Skybox,
        ui::builtin::FpsDisplay,
    },
    ecs::system::SystemStage,
    prelude::*,
    renderer::{
        compute::hdr_loader::HdrLoader,
        pass::{sky::SKYBOX_CUBEMAP_SIZE, Pass},
    },
};

use self::{
    camera::{FollowCameraController, FollowCameraMovement, FollowCameraUpdate},
    materials::{Metal, Wood, WoodTile},
    player::{PlayerInput, PlayerMovement},
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
}

#[system(WindowUpdate)]
fn window_update(mut window: ResMut<Window>, input: Res<Input>) {
    window.fps_mode = input.mouse_button_pressed(MouseButton::Right);
}

#[system(UiUpdate)]
fn ui_update(
    commands: Commands,
    mut asset_server: ResMut<AssetServer>,
    mut ctx: ResMut<EguiContext>,
    mut fps_display: ResMut<FpsDisplay>,
    mut renderer: ResMut<Renderer>,
    mut state: ResMut<State>,
    mut point_lights: Query<&mut PointLight>,
    mut wood_tiles: Query<&mut Material, With<WoodTile>>,
    mut metals: Query<&mut Material, With<Metal>>,
    mut woods: Query<&mut Material, With<Wood>>,
) {
    ctx.draw_if_ready(|ctx| {
        fps_display.run_ui(ctx);

        let mut n_npcs = state.npcs.len();

        let (mut wood_roughness, mut wood_metallic) = woods
            .iter()
            .next()
            .map(|x| (x.roughness, x.metallic))
            .unwrap_or((0.0, 0.0));

        let (mut tile_roughness, mut tile_metallic) = wood_tiles
            .iter()
            .next()
            .map(|x| (x.roughness, x.metallic))
            .unwrap_or((0.0, 0.0));

        let (mut metal_roughness, mut metal_metallic) = metals
            .iter()
            .next()
            .map(|x| (x.roughness, x.metallic))
            .unwrap_or((0.0, 0.0));

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

                ui.heading("Materials");

                ui.label("Wood");
                ui.add(egui::Slider::new(&mut wood_roughness, 0.0..=1.0).text("Roughness"));
                ui.add(egui::Slider::new(&mut wood_metallic, 0.0..=1.0).text("Metallic"));

                ui.label("Tile");
                ui.add(egui::Slider::new(&mut tile_roughness, 0.0..=1.0).text("Roughness"));
                ui.add(egui::Slider::new(&mut tile_metallic, 0.0..=1.0).text("Metallic"));

                ui.label("Metal");
                ui.add(egui::Slider::new(&mut metal_roughness, 0.0..=1.0).text("Roughness"));
                ui.add(egui::Slider::new(&mut metal_metallic, 0.0..=1.0).text("Metallic"));

                ui.heading("NPCs");
                ui.add(egui::Slider::new(&mut n_npcs, 0..=MAX_MESHES - 2).text("Count"));
            });

        if shadow_pass_enabled != renderer.shadow_pass.is_enabled() {
            if shadow_pass_enabled {
                renderer.shadow_pass.enable();
            } else {
                renderer.shadow_pass.disable();
            }
        }

        for mut material in woods.iter() {
            material.roughness = wood_roughness;
            material.metallic = wood_metallic;
        }

        for mut material in wood_tiles.iter() {
            material.roughness = tile_roughness;
            material.metallic = tile_metallic;
        }

        for mut material in metals.iter() {
            material.roughness = metal_roughness;
            material.metallic = metal_metallic;
        }

        if n_lights != state.lights.len() {
            for light in state.lights.drain(..) {
                commands.despawn(light);
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

            let mut rng = rand::thread_rng();
            for _i in 0..n_lights {
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
                commands.despawn(npc);
            }
            let npc_mesh = asset_server.load_mesh("meshes/monkey_2x.glb").unwrap();
            let npc_materials = [
                // asset_server.load_material("materials/wood.glb").unwrap(),
                asset_server.load_material("materials/metal.glb").unwrap(),
            ];
            let mut rng = rand::thread_rng();
            for _ in 0..n_npcs {
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
    query.par_iter().for_each(|mut transform| {
        let mut rotation = transform.get_rotation();
        rotation *= Quat::from_rotation_y(time.delta_seconds * rand::random::<f32>());
        transform.set_rotation(rotation);
    })
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
    renderer.sky_pass.disable();

    let skybox = Skybox {
        texture: asset_server.load_hdr_cubemap(
            "meadow_2k.hdr",
            SKYBOX_CUBEMAP_SIZE,
            &hdr_loader,
        )?,
    };
    commands.spawn(skybox)?;

    let mut material = asset_server.load_material("materials/wood_tiles.glb")?;
    material.texture_scaling = 100.0;
    material.metallic = 0.0;
    material.roughness = 0.5;

    let ground = maps::GroundBundle {
        transform: Transform::from_scale_rotation_translation(
            Vec3::new(100.0, 1.0, 100.0),
            Quat::IDENTITY,
            Vec3::new(0.0, -1.0, 0.0),
        ),
        mesh: asset_server.load_mesh("meshes/cube.obj")?,
        material,
        ground: maps::Ground,
    };
    commands.spawn((ground, WoodTile))?;

    let material = asset_server.load_material("materials/wood.glb")?;

    let player = player::Player {
        speed: 14.0,
        rotation_speed: 0.5,
        velocity: Vec3::ZERO,
    };
    let player = commands.spawn((
        player::PlayerBundle {
            player,
            transform: Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
            mesh: asset_server.load_mesh("meshes/monkey_2x.glb")?,
            material,
        },
        Wood,
    ))?;

    let camera_controller = FollowCameraController {
        stiffness: 50.0,
        target: player,
        pitch_sensitivity: 0.5,
        ..Default::default()
    };
    commands.spawn((camera_controller, Camera::default()))?;
}

#[system(Setup_2)]
fn setup_2(
    commands: Commands,
    mut asset_server: ResMut<AssetServer>,
    mut renderer: ResMut<Renderer>,
    hdr_loader: Res<HdrLoader>,
) {
    renderer.shadow_pass.disable();
    renderer.sky_pass.disable();

    let skybox = Skybox {
        texture: asset_server.load_hdr_cubemap(
            "meadow_2k.hdr",
            SKYBOX_CUBEMAP_SIZE,
            &hdr_loader,
        )?,
    };
    commands.spawn(skybox)?;

    let material = asset_server.load_material("materials/wood.glb").unwrap();
    let mesh = asset_server.load_mesh("meshes/monkey_2x.glb").unwrap();

    let range = 30.0;
    let count = 30000;

    for _ in 0..count {
        let x = rand::thread_rng().gen_range(-range..=range);
        let z = rand::thread_rng().gen_range(-range..=range);
        let y = rand::thread_rng().gen_range(-range..=range);

        commands
            .spawn((
                Transform::from_translation(Vec3::new(x, y, z)),
                mesh.clone(),
                material.clone(),
                Wood,
            ))
            .unwrap();
    }

    let view_matrix = Mat4::look_at_rh(
        Vec3::new(range, range, range),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    );

    let proj_matrix = Mat4::perspective_rh(90.0_f32.to_radians(), 1600.0 / 900.0, 0.1, 100.0);

    commands
        .spawn(Camera::new(view_matrix, proj_matrix))
        .unwrap();

    commands.spawn(PointLight::new(
        Vec3::new(0.0, 0.0, 0.0),
        Color::WHITE,
        10.0,
        30.0,
    ))?;
}

#[system(FpsDisplayUpdate)]
fn fps_display(mut fps_display: ResMut<FpsDisplay>, mut ctx: ResMut<EguiContext>) {
    ctx.draw_if_ready(|ctx| {
        fps_display.run_ui(ctx);
    });
}

#[derive(Debug, clap::Parser)]
struct Args {
    /// Window width
    #[arg(long, default_value = "1600")]
    pub width: usize,
    /// Window height
    #[arg(long, default_value = "900")]
    pub height: usize,
    /// World file (requires `serde` feature)
    #[arg(long)]
    pub world: Option<PathBuf>,
}

pub fn run() -> anyhow::Result<()> {
    let args = Args::parse();

    #[cfg(feature = "serde")]
    {
        if args.world.is_some() {
            log::warn!("Loading worlds is incomplete, expect errors");
        }
    }
    #[cfg(not(feature = "serde"))]
    {
        if args.world.is_some() {
            panic!("`serde` cargo feature is required to load worlds\nRun `cargo run --features serde`");
        }
    }

    let app = App::new(
        args.width,
        args.height,
        #[cfg(feature = "serde")]
        args.world,
    )?;

    app.insert_resource(FpsDisplay::new())?;

    // app.insert_resource(State {
    //     light_intensity: 10.0,
    //     light_radius: 30.0,
    //     ..Default::default()
    // })?;

    app.add_system_to_stage(Setup_2, SystemStage::Startup);
    // app.add_system_to_stage(Setup, SystemStage::Startup);

    // app.add_system(WindowUpdate);
    // app.add_system(ParticleUpdate);
    // app.add_system(FollowCameraUpdate);
    // app.add_system(FollowCameraMovement);
    app.add_system(FpsDisplayUpdate);
    // app.add_system(UiUpdate);
    // app.add_system(PlayerInput);
    // app.add_system(PlayerMovement);
    // app.add_system(SpinNpcs);
    // app.add_system(DebugLights);

    app.run()
}
