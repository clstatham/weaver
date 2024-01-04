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
use renderer::compute::picking::ScreenPicker;
use winit::event::VirtualKeyCode;

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
    editor: ResMut<EditorState>,
    renderer: Res<Renderer>,
    camera: Res<FlyCamera>,
    input: Res<Input>,
    mut doodads: ResMut<Doodads>,
    meshes_transforms: Query<(Read<Mesh>, Write<Transform>)>,
) {
    if input.is_mouse_button_pressed(winit::event::MouseButton::Left) {
        if let Some(mouse_position) = input.mouse_position() {
            let result = picker.pick(mouse_position, &renderer, &camera).unwrap();

            if let Some(result) = result {
                let ray_origin = camera.translation;
                let ray_direction = (result.position - ray_origin).normalize();

                if editor.selected_entity.is_none() {
                    let mut entities_by_distance = Vec::new();
                    for entity in meshes_transforms.entities() {
                        let (mesh, transform) = meshes_transforms.get(entity).unwrap();
                        let aabb = mesh.aabb().transformed(*transform);
                        if let Some(distance) = aabb.intersect_ray(ray_origin, ray_direction) {
                            entities_by_distance.push((entity, distance));
                        }
                    }
                    entities_by_distance.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                    if let Some((entity, _)) = entities_by_distance.first() {
                        editor.selected_entity = Some(*entity);

                        let (mesh, transform) = meshes_transforms.get(*entity).unwrap();
                        let aabb = mesh.aabb().transformed(*transform);

                        editor.selection_plane = Some(Plane {
                            position: aabb.center(),
                            normal: (aabb.center() - ray_origin).normalize(),
                        });
                    }
                }

                if let (Some(entity), Some(plane)) =
                    (editor.selected_entity, editor.selection_plane)
                {
                    let (mesh, mut transform) = meshes_transforms.get(entity).unwrap();
                    let aabb = mesh.aabb().transformed(*transform);
                    let aabb_center_delta = aabb.center() - transform.get_translation();

                    let mut color = Color::WHITE;

                    // blender-style translation
                    if input.is_key_pressed(VirtualKeyCode::G) {
                        let distance = ray_direction.dot(aabb.center() - ray_origin);
                        let mut translation = ray_origin + ray_direction * distance;

                        transform.set_translation(translation + aabb_center_delta);

                        color = Color::GREEN;
                    }
                    // blender-style rotation
                    if input.is_key_pressed(VirtualKeyCode::R) {
                        // get the intersection of the selection plane with the ray
                        let distance = (plane.position - ray_origin).dot(plane.normal)
                            / ray_direction.dot(plane.normal);
                        let intersection = ray_origin + ray_direction * distance;

                        // get the vector from the plane's center to the intersection
                        let mut vector = intersection - plane.position;

                        if let Some(rotation_reference_vector) = editor.rotation_reference_vector {
                            // get the angle between the reference vector and the intersection vector
                            let mut angle = vector.angle_between(rotation_reference_vector);

                            // get the cross product of the reference vector and the intersection vector
                            let cross = rotation_reference_vector.cross(vector);

                            // get the dot product of the cross product and the plane's normal
                            let dot = cross.dot(plane.normal);

                            // if the dot product is negative, the angle is negative
                            if dot < 0.0 {
                                angle = -angle;
                            }

                            // rotate the transform by the angle
                            let (scale, rotation, translation) =
                                transform.matrix.to_scale_rotation_translation();
                            let rotation =
                                glam::Quat::from_axis_angle(plane.normal, angle) * rotation;
                            transform.matrix = glam::Mat4::from_scale_rotation_translation(
                                scale,
                                rotation,
                                translation,
                            );
                        }

                        // set the reference vector to the intersection vector
                        editor.rotation_reference_vector = Some(vector);

                        color = Color::RED;
                    }
                    // blender-style scale
                    if input.is_key_pressed(VirtualKeyCode::C) {
                        // get the intersection of the selection plane with the ray
                        let distance = (plane.position - ray_origin).dot(plane.normal)
                            / ray_direction.dot(plane.normal);
                        let intersection = ray_origin + ray_direction * distance;

                        // get the vector from the plane's center to the intersection
                        let mut vector = intersection - plane.position;

                        // get the scaling amount from the vector's length
                        let scale = vector.length();

                        let (current_scale, rotation, translation) =
                            transform.matrix.to_scale_rotation_translation();

                        // set the transform's scale to the scaling amount
                        transform.matrix = glam::Mat4::from_scale_rotation_translation(
                            current_scale.normalize() * scale,
                            rotation,
                            translation,
                        );

                        color = Color::BLUE;
                    }

                    let aabb = mesh.aabb().transformed(*transform);
                    doodads.push(Doodad::Cube(Cube::new(
                        aabb.center(),
                        glam::Quat::IDENTITY,
                        glam::Vec3::ONE * 0.3,
                        color,
                    )));
                }
            }
        }
    } else {
        editor.selected_entity = None;
        editor.selection_plane = None;
        editor.rotation_reference_vector = None;
    }
}

#[derive(Clone, Copy)]
struct Plane {
    pub position: glam::Vec3,
    pub normal: glam::Vec3,
}

#[derive(Resource, Default)]
struct EditorState {
    pub selected_entity: Option<Entity>,
    pub selection_plane: Option<Plane>,
    pub rotation_reference_vector: Option<glam::Vec3>,
}

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
    commands.insert_resource(EditorState::default())?;

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

    app.run()?;

    Ok(())
}
