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
    physics::{Physics, RapierContext, RigidBody},
    ui::builtin::FpsUi,
};

use prelude::*;
use rapier3d::{dynamics::RigidBodyBuilder, geometry::ColliderBuilder, pipeline::QueryFilter};
use renderer::compute::picking::ScreenPicker;
use winit::event::{MouseButton, VirtualKeyCode};

#[derive(Component)]
struct Handle;

#[derive(Clone, Copy)]
struct Plane {
    pub position: glam::Vec3,
    pub normal: glam::Vec3,
}

#[derive(Resource, Default)]
struct EditorState {
    pub selected_entity: Option<Entity>,
    pub selection_plane: Option<Plane>,
    pub grabby_thing: Option<Entity>,
    pub grabby_radius: f32,
}

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

#[system(CameraUpdate)]
fn camera_update(mut camera: ResMut<FlyCamera>, time: Res<Time>, input: Res<Input>) {
    camera.update(&input, time.delta_time);
}

#[system(UiUpdate)]
fn ui_update(mut ctx: ResMut<EguiContext>, mut fps_ui: Query<&mut FpsUi>) {
    ctx.draw_if_ready(|ctx| {
        for mut fps_ui in fps_ui.iter() {
            fps_ui.run_ui(ctx);
        }
    });
}

#[system(SpawnStuff)]
fn spawn_stuff(
    commands: Commands,
    mut asset_server: ResMut<AssetServer>,
    mut picker: ResMut<ScreenPicker>,
    renderer: Res<Renderer>,
    camera: Res<FlyCamera>,
    mut input: ResMut<Input>,
) {
    if input.key_just_pressed(VirtualKeyCode::F) {
        if let Some(mouse_pos) = input.mouse_position() {
            if let Some(result) = picker.pick(mouse_pos, &renderer, &camera).unwrap() {
                let texture_scaling = 2.0;
                let material = Materials::Metal
                    .load(&mut asset_server, texture_scaling)
                    .unwrap();
                let mesh = asset_server.load_mesh("assets/meshes/cube.glb").unwrap();

                commands.spawn((
                    mesh.clone(),
                    material.clone(),
                    RigidBody::new(
                        RigidBodyBuilder::dynamic()
                            .position(result.position.into())
                            .angular_damping(0.99)
                            .linear_damping(0.9)
                            .build(),
                        ColliderBuilder::cuboid(1.0, 1.0, 1.0).build(),
                        glam::Vec3::ONE,
                    ),
                ))?;
            }
        }
    }
}

#[system(Ropes)]
fn ropes(
    mut doodads: ResMut<Doodads>,
    mut picker: ResMut<ScreenPicker>,
    renderer: Res<Renderer>,
    camera: Res<FlyCamera>,
    mut editor_state: ResMut<EditorState>,
    mut input: ResMut<Input>,
    commands: Commands,
    mut physics: ResMut<RapierContext>,
    query: Query<&mut RigidBody>,
) {
    if input.mouse_button_pressed(MouseButton::Left) {
        if let Some(mouse_pos) = input.mouse_position() {
            if let Some(result) = picker.pick(mouse_pos, &renderer, &camera)? {
                if let Some(grabby_thing) = editor_state.grabby_thing {
                    if let Some(mut rb) = query.get(grabby_thing) {
                        let rb_handle = rb.body_handle(&mut physics);
                        let mut body = physics.bodies.get_mut(rb_handle).unwrap();

                        let ray_origin = camera.translation;
                        let ray_direction = (result.position - ray_origin).normalize();

                        let intersection = ray_origin + ray_direction * editor_state.grabby_radius;

                        body.set_position(intersection.into(), true);

                        doodads.push(Doodad::Cube(Cube::new(
                            result.position,
                            glam::Quat::IDENTITY,
                            glam::Vec3::ONE * 0.2,
                            Color::MAGENTA,
                        )));
                    }
                } else {
                    let ray_origin = camera.translation;
                    let ray_direction = (result.position - ray_origin).normalize();
                    let ray = rapier3d::geometry::Ray::new(ray_origin.into(), ray_direction.into());
                    if let Some((collider, t)) = physics.cast_ray(ray, f32::MAX, QueryFilter::new())
                    {
                        #[allow(clippy::manual_filter)]
                        let body = query.entities().find_map(|entity| {
                            if let Some(mut rb) = query.get(entity) {
                                if rb.collider_handle(&mut physics) == collider {
                                    Some(rb)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        });

                        if let Some(mut body) = body {
                            let mut grabby_rb = RigidBody::new_no_collisions(
                                RigidBodyBuilder::dynamic()
                                    .position(result.position.into())
                                    .build(),
                                glam::Vec3::ONE,
                            );
                            body.add_rope_joint(&mut grabby_rb, &mut physics);
                            let grabby_thing = commands.spawn((Handle, grabby_rb))?;

                            editor_state.grabby_thing = Some(grabby_thing);

                            let mut plane = Plane {
                                position: result.position,
                                normal: (camera.translation - result.position).normalize(),
                            };

                            editor_state.selection_plane = Some(plane);
                            editor_state.grabby_radius = t;
                        }
                    }
                }
            }
        }
    } else {
        if let Some(grabby_thing) = editor_state.grabby_thing {
            commands.remove_entity(grabby_thing);
        }
        editor_state.grabby_thing = None;
        editor_state.selection_plane = None;
    }
}

#[system(Setup)]
fn setup(commands: Commands, mut asset_server: ResMut<AssetServer>) -> anyhow::Result<()> {
    commands.spawn(FpsUi::new())?;

    commands.spawn(PointLight::new(
        glam::Vec3::new(5.0, 5.0, 5.0),
        core::color::Color::WHITE,
        10.0,
    ))?;

    let room_scale = 30.0;

    // floor
    let mesh = asset_server.load_mesh("assets/meshes/cube.glb").unwrap();
    let material = Materials::WoodTile
        .load(&mut asset_server, room_scale)
        .unwrap();
    commands.spawn((
        mesh,
        material,
        RigidBody::new(
            RigidBodyBuilder::fixed()
                .position(glam::Vec3::new(0.0, -2.0, 0.0).into())
                .build(),
            ColliderBuilder::cuboid(room_scale, 1.0, room_scale).build(),
            glam::Vec3::new(room_scale, 1.0, room_scale),
        ),
    ))?;

    let texture_scaling = 2.0;
    let material = Materials::Metal
        .load(&mut asset_server, texture_scaling)
        .unwrap();
    let mesh = asset_server.load_mesh("assets/meshes/cube.glb").unwrap();

    commands.spawn((
        mesh.clone(),
        material.clone(),
        RigidBody::new(
            RigidBodyBuilder::dynamic()
                .position(glam::Vec3::new(0.0, 5.0, 0.0).into())
                .rotation(glam::Vec3::new(10.0f32.to_radians(), 0.0, 10.0f32.to_radians()).into())
                .angular_damping(0.99)
                .linear_damping(0.9)
                .build(),
            ColliderBuilder::cuboid(1.0, 1.0, 1.0).build(),
            glam::Vec3::ONE,
        ),
    ))?;
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut app = App::new(1600, 900)?;

    let picker = ScreenPicker::new(&*app.world().read_resource()?);
    app.insert_resource(picker)?;
    app.insert_resource(EditorState::default())?;
    app.insert_resource(RapierContext::new(glam::Vec3::new(0.0, -9.81, 0.0)))?;

    app.add_startup_system(Setup);

    app.add_system(Physics);
    app.add_system(UiUpdate);
    app.add_system(CameraUpdate);
    app.add_system(Ropes);
    app.add_system(SpawnStuff);

    app.run()?;

    Ok(())
}
