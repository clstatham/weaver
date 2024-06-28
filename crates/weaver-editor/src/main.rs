use inspect::InspectUi;
use transform_gizmo::TransformGizmo;
use weaver::{
    prelude::*,
    weaver_app::App,
    weaver_core::{input::InputPlugin, mesh::Mesh, time::TimePlugin},
    weaver_pbr::{material::Material, PbrPlugin},
    weaver_renderer::{camera::Camera, RendererPlugin},
    weaver_winit::WinitPlugin,
};
use weaver_asset::AssetLoader;
use weaver_core::CoreTypesPlugin;
use weaver_diagnostics::frame_time::LogFrameTimePlugin;
use weaver_egui::prelude::*;
use weaver_renderer::{camera::PrimaryCamera, clear_color::ClearColorPlugin};
use weaver_winit::Window;

pub mod camera;
pub mod inspect;
pub mod transform_gizmo;

#[derive(Component, Reflect)]
struct Floor;

#[derive(Component, Reflect)]
struct Object;

#[derive(Component, Reflect)]
struct SelectionAabb {
    pub aabb: Aabb,
}

impl SelectionAabb {
    pub fn from_mesh(mesh: &Mesh) -> Self {
        let aabb = mesh.aabb;
        Self { aabb }
    }
}

#[derive(Resource, Default)]
struct EditorState {
    pub selected_entity: Option<Entity>,
}

fn main() -> Result<()> {
    env_logger::init();

    App::new()
        .add_plugin(CoreTypesPlugin)?
        .add_plugin(WinitPlugin {
            initial_size: (1280, 720),
        })?
        .add_plugin(TimePlugin)?
        .add_plugin(InputPlugin)?
        .add_plugin(RendererPlugin)?
        .add_plugin(PbrPlugin)?
        .add_plugin(GizmoPlugin)?
        .add_plugin(EguiPlugin)?
        .add_plugin(LogFrameTimePlugin {
            log_interval: std::time::Duration::from_secs(1),
        })?
        .add_plugin(ClearColorPlugin(Color::new(0.1, 0.1, 0.1, 1.0)))?
        .insert_resource(Skybox::new("assets/skyboxes/meadow_2k.hdr"))
        .insert_resource(EditorState::default())
        .insert_resource(TransformGizmo {
            focus: None,
            size: 1.0,
            axis_size: 0.1,
            handle_size: 0.3,
            middle_color: Color::WHITE,
            x_color: Color::RED,
            y_color: Color::GREEN,
            z_color: Color::BLUE,
            extra_scaling: 1.0,
            desired_pixel_size: 100.0,
        })
        .add_system(setup, Init)
        .add_system(camera::update_camera, Update)
        .add_system(camera::update_aspect_ratio, Update)
        .add_system(selection_gizmos, Update)
        .add_system(light_gizmos, Update)
        .add_system(pick_entity, Update)
        .add_system(transform_gizmo::draw_transform_gizmo, Update)
        .add_system(inspect_ui, Update)
        .run()
}

fn setup(
    commands: Commands,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    mut mesh_loader: AssetLoader<Mesh, MeshLoader>,
    mut material_assets: ResMut<Assets<Material>>,
    mut material_loader: AssetLoader<Material, MaterialLoader>,
) -> Result<()> {
    commands.spawn((
        Camera::perspective_lookat(
            Vec3::new(10.0, 10.0, 10.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0f32.to_radians(),
            1280.0 / 720.0,
            0.1,
            100.0,
        ),
        *camera::FlyCameraController {
            aspect: 1280.0 / 720.0,
            ..Default::default()
        }
        .look_at(Vec3::new(10.0, 10.0, 10.0), Vec3::ZERO, Vec3::Y),
        PrimaryCamera,
    ));

    let cube_mesh = mesh_assets.insert(mesh_loader.load("assets/meshes/cube.obj")?);
    let monkey_mesh = mesh_assets.insert(mesh_loader.load("assets/meshes/sphere.obj")?);

    let mut material = material_loader.load("assets/materials/wood_tiles.glb")?;
    material.texture_scale = 100.0;
    let material = material_assets.insert(material);

    commands.spawn((
        cube_mesh,
        material,
        Transform {
            translation: Vec3A::new(0.0, -1.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3A::new(20.0, 1.0, 20.0),
        },
        Floor,
        SelectionAabb::from_mesh(&mesh_assets.get(cube_mesh).unwrap()),
    ));

    // // circle of lights
    // const COLORS: &[Color] = &[
    //     Color::RED,
    //     Color::GREEN,
    //     Color::BLUE,
    //     Color::YELLOW,
    //     Color::CYAN,
    //     Color::MAGENTA,
    // ];
    // for (i, color) in COLORS.iter().enumerate() {
    //     let angle = i as f32 / COLORS.len() as f32 * std::f32::consts::PI * 2.0;
    //     world.spawn((
    //         PointLight {
    //             color: *color,
    //             intensity: 10.0,
    //             radius: 10.0,
    //         },
    //         Transform {
    //             translation: Vec3::new(angle.cos() * 5.0, 5.0, angle.sin() * 5.0),
    //             rotation: Quat::IDENTITY,
    //             scale: Vec3::ONE,
    //         },
    //         SelectionAabb {
    //             aabb: Aabb::new(Vec3::splat(-0.1), Vec3::splat(0.1)),
    //         },
    //     ));
    // }

    commands.spawn((
        PointLight {
            color: Color::WHITE,
            intensity: 100.0,
            radius: 100.0,
            enabled: true,
        },
        Transform {
            translation: Vec3A::new(0.0, 5.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3A::ONE,
        },
        SelectionAabb {
            aabb: Aabb::new(Vec3A::splat(-0.1), Vec3A::splat(0.1)),
        },
    ));

    let mut material2 = material_loader.load("assets/materials/metal.glb")?;
    material2.texture_scale = 20.0;
    let material2 = material_assets.insert(material2);

    // spawn some meshes
    let count = 10;
    for i in 0..count {
        let angle = i as f32 / count as f32 * std::f32::consts::PI * 2.0;
        commands.spawn((
            monkey_mesh,
            material2,
            Transform {
                translation: Vec3A::new(angle.cos() * 5.0, 2.0, angle.sin() * 5.0),
                rotation: Quat::IDENTITY,
                scale: Vec3A::splat(1.0),
            },
            Object,
            SelectionAabb::from_mesh(&mesh_assets.get(cube_mesh).unwrap()),
        ));
    }

    Ok(())
}

fn selection_gizmos(
    query: Query<(&Transform, &SelectionAabb)>,
    gizmos: Res<Gizmos>,
    state: Res<EditorState>,
) -> Result<()> {
    for (entity, (transform, aabb)) in query.iter() {
        if let Some(selected_entity) = state.selected_entity {
            if selected_entity == entity {
                let aabb = aabb.aabb.transformed(*transform);
                let gizmo_transform = Transform::new(
                    aabb.center(),
                    Quat::IDENTITY,
                    aabb.size() + Vec3A::splat(0.1),
                );
                gizmos.wire_cube_no_depth(gizmo_transform, Color::GREEN);
            }
        }
    }

    Ok(())
}

fn light_gizmos(
    query: Query<(&PointLight, &Transform, &SelectionAabb)>,
    gizmos: Res<Gizmos>,
) -> Result<()> {
    for (_, (light, transform, aabb)) in query.iter() {
        gizmos.solid_cube(
            Transform::new(transform.translation, Quat::IDENTITY, aabb.aabb.size()),
            light.color,
        );
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn pick_entity(
    window: Res<Window>,
    input: Res<Input>,
    egui_ctx: Res<EguiContext>,
    mut editor_state: ResMut<EditorState>,
    camera_query: Query<&Camera>,
    aabb_transform_query: Query<(&SelectionAabb, Option<&Transform>)>,
    mesh_assets: Res<Assets<Mesh>>,
    mesh_query: Query<(&Handle<Mesh>, Option<&Transform>), With<SelectionAabb>>,
    mut transform_gizmo: ResMut<TransformGizmo>,
) -> Result<()> {
    if input.mouse_just_pressed(MouseButton::Left) && !egui_ctx.wants_input() {
        let cursor_pos = input.mouse_pos();
        let cursor_pos = Vec2::new(cursor_pos.0, cursor_pos.1);
        let (_, camera) = camera_query.iter().next().unwrap();
        let window_size = window.inner_size();
        let ray = camera.screen_to_ray(
            cursor_pos,
            Vec2::new(window_size.width as f32, window_size.height as f32),
        );
        let mut hit_entities = Vec::new();
        for (entity, (aabb, transform)) in aabb_transform_query.iter() {
            let bb = if let Some(transform) = transform {
                aabb.aabb.transformed(*transform)
            } else {
                aabb.aabb
            };

            if let Some(distance) = ray.intersect(&bb) {
                hit_entities.push((entity, distance));
            }
        }

        hit_entities.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        // check for mesh hit
        let mut hit_entity = hit_entities.first().copied();
        for (entity, _) in hit_entities {
            if let Some((handle, transform)) = mesh_query.get(entity) {
                let mesh = mesh_assets.get(*handle).unwrap();
                let mesh = if let Some(transform) = transform {
                    mesh.transformed(*transform)
                } else {
                    mesh.clone()
                };

                if let Some(intersection) = mesh.intersect(&ray) {
                    let t = intersection.ray_triangle_intersection.t;
                    if let Some((_, distance)) = hit_entity {
                        if t < distance {
                            hit_entity = Some((entity, t));
                        }
                    } else {
                        hit_entity = Some((entity, t));
                    }
                }
            }
        }

        if let Some((entity, _)) = hit_entity {
            editor_state.selected_entity = Some(entity);
            transform_gizmo.focus = Some(entity);
        } else {
            editor_state.selected_entity = None;
            transform_gizmo.focus = None;
        }
    }

    Ok(())
}

fn inspect_ui(
    world: WorldMut,
    type_registry: Res<TypeRegistry>,
    editor_state: Res<EditorState>,
    egui_ctx: Res<EguiContext>,
) -> Result<()> {
    let world = world.into_inner();

    egui_ctx.draw_if_ready(|ctx| {
        if let Some(selected_entity) = editor_state.selected_entity {
            egui::Window::new("Inspector").show(ctx, |ui| {
                let storage = world.storage();
                let archetype = storage.get_archetype(selected_entity).unwrap();
                for (_, column) in archetype.column_iter() {
                    let column = column.into_inner();
                    let component = column.get(selected_entity.as_usize()).unwrap();
                    let component = unsafe { &mut *component.get() };
                    let component = component.get_data_mut();
                    let reflect = component.as_reflect_mut();
                    let world = unsafe { world.as_unsafe_world_cell().world_mut() };
                    reflect.inspect_ui(&type_registry, world, ui);
                }
            });
        }
    });

    Ok(())
}
