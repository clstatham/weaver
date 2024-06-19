use inspect::InspectUi;
use weaver::{
    prelude::*,
    weaver_app::App,
    weaver_core::{input::InputPlugin, mesh::Mesh, time::TimePlugin},
    weaver_ecs::world::World,
    weaver_pbr::{camera::PbrCamera, material::Material, PbrPlugin},
    weaver_renderer::{camera::Camera, RendererPlugin},
    weaver_winit::WinitPlugin,
};
use weaver_core::CoreTypesPlugin;
use weaver_diagnostics::frame_time::LogFrameTimePlugin;
use weaver_egui::prelude::*;
use weaver_renderer::camera::PrimaryCamera;
use weaver_winit::Window;

pub mod camera;
pub mod inspect;

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
        .add_plugin(AssetPlugin)?
        .add_plugin(RendererPlugin)?
        .add_plugin(PbrPlugin)?
        .add_plugin(GizmoPlugin)?
        .add_plugin(EguiPlugin)?
        .add_plugin(LogFrameTimePlugin {
            log_interval: std::time::Duration::from_secs(1),
        })?
        .insert_resource(EditorState::default())
        .add_system(setup, Init)
        .add_system(camera::update_camera, Update)
        .add_system(camera::update_aspect_ratio, Update)
        .add_system(selection_gizmos, Update)
        .add_system(light_gizmos, Update)
        .add_system(pick_entity, Update)
        .add_system(ui, Update)
        .run()
}

fn setup(world: &mut World) -> Result<()> {
    let _camera = world.spawn((
        Camera::perspective_lookat(
            Vec3::new(10.0, 10.0, 10.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0f32.to_radians(),
            1280.0 / 720.0,
            0.1,
            100.0,
        ),
        PbrCamera::new(Color::new(0.1, 0.1, 0.1, 1.0)),
        *camera::FlyCameraController {
            aspect: 1280.0 / 720.0,
            ..Default::default()
        }
        .look_at(Vec3::new(10.0, 10.0, 10.0), Vec3::ZERO, Vec3::Y),
        PrimaryCamera,
    ));

    let mut assets = world.get_resource_mut::<Assets>().unwrap();

    let mesh = assets.load::<Mesh>("assets/meshes/cube.obj")?;

    let material = assets.load::<Material>("assets/materials/metal.glb")?;
    {
        let mut material = assets.get_mut(material).unwrap();
        material.texture_scale = 100.0;
    }

    let _ground = world.spawn((
        mesh,
        material,
        Transform {
            translation: Vec3::new(0.0, -1.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(20.0, 1.0, 20.0),
        },
        Floor,
        SelectionAabb::from_mesh(&assets.get(mesh).unwrap()),
    ));

    // circle of lights
    const COLORS: &[Color] = &[
        Color::RED,
        Color::GREEN,
        Color::BLUE,
        Color::YELLOW,
        Color::CYAN,
        Color::MAGENTA,
    ];
    for (i, color) in COLORS.iter().enumerate() {
        let angle = i as f32 / COLORS.len() as f32 * std::f32::consts::PI * 2.0;
        let _light = world.spawn((
            PointLight {
                color: *color,
                intensity: 100.0,
                radius: 100.0,
            },
            Transform {
                translation: Vec3::new(angle.cos() * 5.0, 5.0, angle.sin() * 5.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
            SelectionAabb {
                aabb: Aabb::new(Vec3::splat(-0.1), Vec3::splat(0.1)),
            },
        ));
    }

    world.spawn((
        PointLight {
            color: Color::WHITE,
            intensity: 100.0,
            radius: 100.0,
        },
        Transform {
            translation: Vec3::new(0.0, 5.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
        SelectionAabb {
            aabb: Aabb::new(Vec3::splat(-0.1), Vec3::splat(0.1)),
        },
    ));

    let material2 = assets.load::<Material>("assets/materials/metal.glb")?;
    {
        let mut material = assets.get_mut(material2).unwrap();
        material.texture_scale = 20.0;
    }

    // spawn some meshes
    let count = 10;
    for i in 0..count {
        let angle = i as f32 / count as f32 * std::f32::consts::PI * 2.0;
        let _mesh = world.spawn((
            mesh,
            material2,
            Transform {
                translation: Vec3::new(angle.cos() * 5.0, 2.0, angle.sin() * 5.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.5),
            },
            Object,
            SelectionAabb::from_mesh(&assets.get(mesh).unwrap()),
        ));
    }

    Ok(())
}

fn selection_gizmos(
    query: Query<(&Transform, &Handle<Mesh>)>,
    gizmos: Res<Gizmos>,
    state: Res<EditorState>,
    assets: Res<Assets>,
) -> Result<()> {
    for (entity, (transform, handle)) in query.iter() {
        if let Some(selected_entity) = state.selected_entity {
            if selected_entity == entity {
                let mesh = &assets.get(*handle).unwrap();
                let aabb = mesh.aabb.transform(*transform);
                let gizmo_transform = Transform::new(
                    aabb.center(),
                    Quat::IDENTITY,
                    aabb.size() + Vec3::splat(0.1),
                );
                gizmos.cube(gizmo_transform, Color::GREEN);
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
        gizmos.cube(
            Transform::new(
                transform.translation,
                Quat::IDENTITY,
                aabb.aabb.size() * 2.0,
            ),
            light.color,
        );
    }

    Ok(())
}

fn ui(world: &mut World) -> Result<()> {
    let editor_state = world.get_resource::<EditorState>().unwrap();
    let egui_context = world.get_resource::<EguiContext>().unwrap();
    let type_registry = world.get_resource::<TypeRegistry>().unwrap();
    let assets = world.get_resource::<Assets>().unwrap();
    egui_context.draw_if_ready(|ctx| {
        if let Some(entity) = editor_state.selected_entity {
            egui::Window::new("Inspector").show(ctx, |ui| {
                let storage = world.storage().read();
                let archetype = storage.get_archetype(entity).unwrap();
                for (_, column) in archetype.column_iter() {
                    let mut column = column.write();
                    let data = column.get_mut(entity.as_usize()).unwrap();
                    let component = data.get_data_mut();
                    let reflect = component.as_reflect_mut();
                    reflect.inspect_ui(&type_registry, &assets, ui);
                }
            });
        };
    });
    Ok(())
}

fn pick_entity(world: &mut World) -> Result<()> {
    let input = world.get_resource::<Input>().unwrap();
    let egui_ctx = world.get_resource::<EguiContext>().unwrap();
    if input.mouse_just_pressed(MouseButton::Left) && !egui_ctx.wants_input() {
        let cursor_pos = input.mouse_pos();
        let cursor_pos = Vec2::new(cursor_pos.0, cursor_pos.1);
        let (_, camera) = world.query::<&Camera>().iter().next().unwrap();
        let window = world.get_resource::<Window>().unwrap();
        let window_size = window.inner_size();
        let ray = camera.screen_to_ray(
            cursor_pos,
            Vec2::new(window_size.width as f32, window_size.height as f32),
        );
        let mut closest_entity = None;
        let mut closest_distance = f32::INFINITY;
        for (entity, aabb) in world.query::<&SelectionAabb>().iter() {
            let bb = if let Some(transform) = world.get_component::<Transform>(entity) {
                aabb.aabb.transform(*transform)
            } else {
                aabb.aabb
            };

            if let Some(distance) = ray.intersect(bb) {
                if distance < closest_distance {
                    closest_distance = distance;
                    closest_entity = Some(entity);
                }
            }
        }

        let mut editor_state = world.get_resource_mut::<EditorState>().unwrap();
        editor_state.selected_entity = closest_entity;
    }

    Ok(())
}
