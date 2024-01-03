pub mod app;
pub mod core;
pub mod ecs;
pub mod renderer;

pub mod prelude {
    pub use crate::app::{asset_server::AssetServer, commands::Commands, App};
    pub use crate::core::{
        camera::FlyCamera,
        color::Color,
        input::Input,
        light::{DirectionalLight, PointLight},
        material::Material,
        mesh::Mesh,
        time::Time,
        transform::Transform,
        ui::EguiContext,
    };
    pub use crate::ecs::*;
    pub use crate::renderer::Renderer;
    pub use weaver_proc_macro::system;
}

use core::{
    doodads::{Cube, Doodad, Doodads},
    ui::builtin::FpsUi,
};

use prelude::*;
use renderer::picking::ScreenPicker;

#[system(CameraUpdate)]
fn camera_update(mut camera: ResMut<FlyCamera>, time: Res<Time>, input: Res<Input>) {
    camera.update(&input, time.delta_time);
}

#[system(UiUpdate)]
fn ui_update(mut ctx: ResMut<EguiContext>, mut fps_ui: Query<Write<FpsUi>>) {
    ctx.draw_if_ready(|ctx| {
        for mut fps_ui in fps_ui.iter() {
            fps_ui.run_ui(ctx);
        }
    });
}

#[system(PickScreen)]
fn pick_screen(
    picker: Res<ScreenPicker>,
    renderer: Res<Renderer>,
    camera: Res<FlyCamera>,
    input: Res<Input>,
    mut doodads: ResMut<Doodads>,
) {
    if input.is_mouse_button_pressed(winit::event::MouseButton::Left) {
        if let Some(mouse_position) = input.mouse_position() {
            let result = picker.pick(mouse_position, &renderer, &camera).unwrap();

            doodads.push(Doodad::Cube(Cube::new(
                result.position,
                glam::Quat::IDENTITY,
                glam::Vec3::new(0.3, 0.3, 0.3),
                Color::RED,
            )));
        }
    }
}

#[system(DoDaDoodads)]
fn do_da_doodads() {}

#[derive(Component)]
struct Object;

#[allow(dead_code)]
enum Materials {
    Wood,
    Metal,
    WoodTile,
    BrickWall,
    StoneWall,
    Banana,
}

impl Materials {
    pub fn load(
        &self,
        asset_server: &mut AssetServer,
        texture_scaling: f32,
    ) -> anyhow::Result<Material> {
        match self {
            // Wood_025
            Materials::Wood => {
                let base_color = asset_server
                    .load_texture("assets/materials/Wood_025_SD/Wood_025_basecolor.jpg", false)?;
                let normal = asset_server
                    .load_texture("assets/materials/Wood_025_SD/Wood_025_normal.jpg", true)?;
                let roughness = asset_server
                    .load_texture("assets/materials/Wood_025_SD/Wood_025_roughness.jpg", false)?;
                let ao = asset_server.load_texture(
                    "assets/materials/Wood_025_SD/Wood_025_ambientOcclusion.jpg",
                    false,
                )?;
                Ok(asset_server.create_material(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                    Some(0.0),
                    Some(0.0),
                    Some(texture_scaling),
                ))
            }
            // Metal_006
            Materials::Metal => {
                let base_color = asset_server.load_texture(
                    "assets/materials/Metal_006_SD/Metal_006_basecolor.jpg",
                    false,
                )?;
                let normal = asset_server
                    .load_texture("assets/materials/Metal_006_SD/Metal_006_normal.jpg", true)?;
                let roughness = asset_server.load_texture(
                    "assets/materials/Metal_006_SD/Metal_006_roughness.jpg",
                    false,
                )?;
                let ao = asset_server.load_texture(
                    "assets/materials/Metal_006_SD/Metal_006_ambientOcclusion.jpg",
                    false,
                )?;
                Ok(asset_server.create_material(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                    Some(1.0),
                    Some(1.0),
                    Some(texture_scaling),
                ))
            }
            // Wood_Herringbone_Tiles_004
            Materials::WoodTile => {
                let base_color = asset_server.load_texture(
                    "assets/materials/Wood_Herringbone_Tiles_004_SD/Substance_Graph_BaseColor.jpg",
                    false,
                )?;
                let normal = asset_server.load_texture(
                    "assets/materials/Wood_Herringbone_Tiles_004_SD/Substance_Graph_Normal.jpg",
                    true,
                )?;
                let roughness = asset_server.load_texture(
                    "assets/materials/Wood_Herringbone_Tiles_004_SD/Substance_Graph_Roughness.jpg",
                    false,
                )?;
                let ao = asset_server.load_texture(
                    "assets/materials/Wood_Herringbone_Tiles_004_SD/Substance_Graph_AmbientOcclusion.jpg",
                    false,
                )?;
                Ok(asset_server.create_material(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                    Some(0.0),
                    Some(0.5),
                    Some(texture_scaling),
                ))
            }
            // Brick_Wall_017
            Materials::BrickWall => {
                let base_color = asset_server.load_texture(
                    "assets/materials/Brick_Wall_017_SD/Brick_Wall_017_basecolor.jpg",
                    false,
                )?;
                let normal = asset_server.load_texture(
                    "assets/materials/Brick_Wall_017_SD/Brick_Wall_017_normal.jpg",
                    true,
                )?;
                let roughness = asset_server.load_texture(
                    "assets/materials/Brick_Wall_017_SD/Brick_Wall_017_roughness.jpg",
                    false,
                )?;
                let ao = asset_server.load_texture(
                    "assets/materials/Brick_Wall_017_SD/Brick_Wall_017_ambientOcclusion.jpg",
                    false,
                )?;
                Ok(asset_server.create_material(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                    Some(0.0),
                    Some(1.0),
                    Some(texture_scaling),
                ))
            }
            // Wall_Stone_021
            Materials::StoneWall => {
                let base_color = asset_server.load_texture(
                    "assets/materials/Wall_Stone_021_SD/Substance_graph_BaseColor.jpg",
                    false,
                )?;
                let normal = asset_server.load_texture(
                    "assets/materials/Wall_Stone_021_SD/Substance_graph_Normal.jpg",
                    true,
                )?;
                let roughness = asset_server.load_texture(
                    "assets/materials/Wall_Stone_021_SD/Substance_graph_Roughness.jpg",
                    false,
                )?;
                let ao = asset_server.load_texture(
                    "assets/materials/Wall_Stone_021_SD/Substance_graph_AmbientOcclusion.jpg",
                    false,
                )?;
                Ok(asset_server.create_material(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                    Some(0.0),
                    Some(1.0),
                    Some(texture_scaling),
                ))
            }
            // Food_0003
            Materials::Banana => {
                let base_color = asset_server
                    .load_texture("assets/materials/Food_0003/food_0003_color_1k.jpg", false)?;
                let normal = asset_server.load_texture(
                    "assets/materials/Food_0003/food_0003_normal_opengl_1k.png",
                    true,
                )?;
                let roughness = asset_server.load_texture(
                    "assets/materials/Food_0003/food_0003_roughness_1k.jpg",
                    false,
                )?;
                let ao = asset_server
                    .load_texture("assets/materials/Food_0003/food_0003_ao_1k.jpg", false)?;
                Ok(asset_server.create_material(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                    Some(0.0),
                    Some(0.0),
                    Some(texture_scaling),
                ))
            }
        }
    }
}

fn setup(commands: &mut Commands, asset_server: &mut AssetServer) -> anyhow::Result<()> {
    let picker = ScreenPicker::new(&*commands.read_resource::<Renderer>()?);
    commands.insert_resource(picker)?;

    let room_scale = 30.0;

    // floor
    let mesh = asset_server.load_mesh("assets/meshes/cube.glb").unwrap();
    let material = Materials::WoodTile.load(asset_server, room_scale).unwrap();
    commands.spawn((
        mesh,
        material,
        Transform::new()
            .scale(room_scale, 1.0, room_scale)
            .translate(0.0, -2.0, 0.0),
    ))?;

    let texture_scaling = 2.0;
    let material = Materials::Metal
        .load(asset_server, texture_scaling)
        .unwrap();
    let mesh = asset_server
        .load_mesh("assets/meshes/monkey_flat.glb")
        .unwrap();

    commands.spawn((mesh.clone(), material.clone(), Transform::new()))?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut app = App::new(1600, 900)?;

    app.spawn(FpsUi::new())?;

    // app.spawn(DirectionalLight::new(
    //     glam::Vec3::new(1.0, -1.0, 1.0).normalize(),
    //     core::color::Color::WHITE,
    //     40.0,
    // ));

    app.spawn((
        PointLight::new(
            glam::Vec3::new(5.0, 5.0, 5.0),
            core::color::Color::WHITE,
            10.0,
        ),
        Object,
    ))?;

    app.build(|commands, asset_server| {
        setup(commands, asset_server)?;
        Ok(())
    })?;

    // app.spawn(PointLight::new(
    //     glam::Vec3::new(0.0, 5.0, 0.0),
    //     core::color::Color::WHITE,
    //     20.0,
    // ));

    app.add_system(UiUpdate);
    app.add_system(CameraUpdate);
    app.add_system(PickScreen);
    app.add_system(DoDaDoodads);

    app.run()?;

    Ok(())
}
