use std::collections::VecDeque;

use camera::CameraUpdate;
use inspect::InspectUi;
use transform_gizmo::TransformGizmo;
use weaver::{
    prelude::*,
    weaver_app::App,
    weaver_core::{input::InputPlugin, mesh::Mesh, time::TimePlugin},
    weaver_pbr::PbrPlugin,
    weaver_renderer::{camera::Camera, RendererPlugin},
    weaver_winit::WinitPlugin,
};
use weaver_asset::loading::Filesystem;
use weaver_core::CoreTypesPlugin;
use weaver_diagnostics::frame_time::{FrameTime, LogFrameTimePlugin};
use weaver_egui::prelude::*;
use weaver_gizmos::GizmoNodeLabel;
use weaver_q3::{
    bsp::{
        loader::{Bsp, BspLoader, BspNode},
        render::BspRenderNodeLabel,
    },
    pk3::Pk3Filesystem,
    shader::loader::LexedShaderCache,
    Q3Plugin,
};
use weaver_renderer::{
    camera::PrimaryCamera, clear_color::ClearColorPlugin, graph::RenderGraphApp, RenderApp,
};
use weaver_winit::Window;

pub mod camera;
pub mod inspect;
pub mod transform_gizmo;

#[derive(Component, Reflect)]
pub struct SelectionAabb {
    pub aabb: Aabb,
}

impl SelectionAabb {
    pub fn from_mesh(mesh: &Mesh) -> Self {
        let aabb = mesh.aabb;
        Self { aabb }
    }
}

#[derive(Default)]
enum VisMode {
    #[default]
    None,
    Nodes,
    Leaves,
}

#[derive(Resource, Default)]
struct EditorState {
    pub selected_entity: Option<Entity>,
    pub vis_mode: VisMode,
}

#[derive(Resource, Default)]
struct FpsHistory {
    pub history: VecDeque<f32>,
}

fn main() -> Result<()> {
    env_logger::init();

    App::new()
        .add_plugin(CoreTypesPlugin)?
        .add_plugin(WinitPlugin {
            initial_size: (1600, 900),
            window_title: "Weaver",
        })?
        .add_plugin(TimePlugin)?
        .add_plugin(InputPlugin)?
        .add_plugin(RendererPlugin)?
        .add_plugin(PbrPlugin)?
        .add_plugin(GizmoPlugin)?
        .add_plugin(EguiPlugin)?
        .add_plugin(Q3Plugin)?
        .add_plugin(LogFrameTimePlugin {
            log_interval: std::time::Duration::from_secs(5),
        })?
        .init_resource::<FpsHistory>()
        .add_plugin(ClearColorPlugin(Color::new(0.1, 0.1, 0.1, 1.0)))?
        .configure_sub_app::<RenderApp>(|app| {
            app.add_render_main_graph_edge(SkyboxNodeLabel, BspRenderNodeLabel);
            app.add_render_main_graph_edge(BspRenderNodeLabel, GizmoNodeLabel);
        })
        .insert_resource(Skybox::new("assets/skyboxes/meadow_2k.hdr"))
        .insert_resource(Filesystem::default().with_pk3s_from_dir("assets/q3")?)
        .insert_resource(EditorState::default())
        .add_plugin(FixedUpdatePlugin::<CameraUpdate>::new(1.0 / 1000.0, 0.1))?
        // .insert_resource(TransformGizmo {
        //     focus: None,
        //     size: 1.0,
        //     axis_size: 0.1,
        //     handle_size: 0.3,
        //     middle_color: Color::WHITE,
        //     x_color: Color::RED,
        //     y_color: Color::GREEN,
        //     z_color: Color::BLUE,
        //     extra_scaling: 1.0,
        //     desired_pixel_size: 100.0,
        // })
        .add_system(load_shaders, Init)
        .add_system_after(setup, load_shaders, Init)
        .add_system(camera::update_camera, Update)
        .add_system(camera::update_aspect_ratio, Update)
        .add_system(selection_gizmos, Update)
        .add_system(toggle_vis_mode, Update)
        .add_system(debug_stuff, Update)
        .add_system(light_gizmos, Update)
        .add_system(pick_entity, Update)
        // .add_system(transform_gizmo::draw_transform_gizmo, Update)
        .add_system(inspect_ui, Update)
        .add_system(fps_ui, Update)
        .run()
}

fn load_shaders(mut fs: ResMut<Filesystem>, mut cache: ResMut<LexedShaderCache>) -> Result<()> {
    cache.load_all("scripts", &mut fs)?;
    let mut shaders = cache.shader_names().collect::<Vec<_>>();
    shaders.sort();
    log::debug!("Loaded shaders: {:#?}", shaders);
    Ok(())
}

fn setup(
    commands: Commands,
    bsp_loader: AssetLoader<Bsp, BspLoader>,
    mut fs: ResMut<Filesystem>,
) -> Result<()> {
    commands.spawn((
        Camera::default(),
        camera::FlyCameraController {
            aspect: 16.0 / 9.0,
            speed: 320.0,
            fov: 70.0f32.to_radians(),
            near: 0.1,
            far: 100000.0,
            sensitivity: 40.0,
            ..Default::default()
        },
        PrimaryCamera,
    ));

    let bsp = bsp_loader.load_from_filesystem(&mut fs, "maps/q3dm6.bsp")?;
    commands.insert_resource(bsp);

    Ok(())
}

fn fps_ui(
    time: Res<FrameTime>,
    mut history: ResMut<FpsHistory>,
    egui_ctx: Res<EguiContext>,
) -> Result<()> {
    egui_ctx.draw_if_ready(|ctx| {
        egui::Window::new("FPS").show(ctx, |ui| {
            history.history.push_back(time.fps);
            if history.history.len() > 1000 {
                history.history.pop_front();
            }

            let plot = egui_plot::Plot::new("FPS");
            let points = history
                .history
                .iter()
                .enumerate()
                .map(|(i, &fps)| [i as f64, fps as f64])
                .collect::<Vec<_>>();
            plot.show(ui, |plot| {
                plot.line(egui_plot::Line::new(points).color(egui::Color32::LIGHT_GREEN));
            });
        });
    });

    Ok(())
}

fn toggle_vis_mode(mut editor_state: ResMut<EditorState>, input: Res<Input>) -> Result<()> {
    if input.key_down(KeyCode::Digit1) {
        editor_state.vis_mode = VisMode::None;
    } else if input.key_down(KeyCode::Digit2) {
        editor_state.vis_mode = VisMode::Leaves;
    } else if input.key_down(KeyCode::Digit3) {
        editor_state.vis_mode = VisMode::Nodes;
    }

    Ok(())
}

fn debug_stuff(
    gizmos: Res<Gizmos>,
    egui_ctx: Res<EguiContext>,
    bsp: Res<Bsp>,
    camera_query: Query<&Camera>,
    editor_state: Res<EditorState>,
) -> Result<()> {
    let (_, camera) = camera_query.iter().next().unwrap();

    let mut total_nodes = 0;
    let mut total_leaves = 0;

    let mut culled_nodes = 0;
    let mut visible_nodes = 0;
    let mut partial_nodes = 0;

    let mut culled_leaves = 0;
    let mut visible_leaves = 0;
    let mut partial_leaves = 0;

    // visualize bsp tree
    for (_, node) in bsp.node_iter() {
        match node {
            BspNode::Leaf { min, max, .. } => {
                total_leaves += 1;
                let color =
                    match camera.intersect_frustum_with_aabb(&Aabb::new(*min, *max), true, false) {
                        Intersection::Inside => {
                            visible_leaves += 1;
                            Color::GREEN
                        }
                        Intersection::Outside => {
                            culled_leaves += 1;
                            Color::RED
                        }
                        Intersection::Intersecting => {
                            partial_leaves += 1;
                            Color::YELLOW
                        }
                    };
                if let VisMode::Leaves = editor_state.vis_mode {
                    gizmos.wire_cube_no_depth(
                        Transform::new((*min + *max) / 2.0, Quat::IDENTITY, (*max - *min).abs()),
                        color,
                    );
                }
            }
            BspNode::Node { min, max, .. } => {
                total_nodes += 1;
                match camera.intersect_frustum_with_aabb(&Aabb::new(*min, *max), true, false) {
                    Intersection::Inside => {
                        visible_nodes += 1;
                    }
                    Intersection::Outside => {
                        culled_nodes += 1;
                    }
                    Intersection::Intersecting => {
                        partial_nodes += 1;
                    }
                }
                if let VisMode::Nodes = editor_state.vis_mode {
                    gizmos.wire_cube_no_depth(
                        Transform::new((*min + *max) / 2.0, Quat::IDENTITY, (*max - *min).abs()),
                        Color::CYAN,
                    );
                }
            }
        }
    }

    egui_ctx.draw_if_ready(|ctx| {
        egui::Window::new("Debug").show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.label(format!("Culled nodes: {}", culled_nodes));
                ui.label(format!("Partial nodes: {}", partial_nodes));
                ui.label(format!("Visible nodes: {}", visible_nodes));
                ui.label(format!("Total nodes: {}", total_nodes));
                ui.separator();
                ui.label(format!("Culled leaves: {}", culled_leaves));
                ui.label(format!("Partial leaves: {}", partial_leaves));
                ui.label(format!("Visible leaves: {}", visible_leaves));
                ui.label(format!("Total leaves: {}", total_leaves));
            });
        });
    });

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
                    aabb.size() + Vec3::splat(0.1),
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
    mut transform_gizmo: Option<ResMut<TransformGizmo>>,
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
            if let Some(transform_gizmo) = transform_gizmo.as_deref_mut() {
                transform_gizmo.focus = Some(entity);
            }
        } else {
            editor_state.selected_entity = None;
            if let Some(transform_gizmo) = transform_gizmo.as_deref_mut() {
                transform_gizmo.focus = None;
            }
        }
    }

    Ok(())
}

fn inspect_ui(
    world: WorldMut,
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

                    let world = unsafe { world.as_unsafe_world_cell().world_mut() };
                    component.inspect_ui(world, ui);
                }
            });
        }
    });

    Ok(())
}
