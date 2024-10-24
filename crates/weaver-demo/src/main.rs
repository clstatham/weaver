use std::{collections::VecDeque, sync::Arc};

use weaver::{
    prelude::*,
    weaver_app::App,
    weaver_core::{input::InputPlugin, time::TimePlugin},
    weaver_pbr::PbrPlugin,
    weaver_renderer::{camera::Camera, RendererPlugin},
    weaver_winit::WinitPlugin,
};
use weaver_asset::Filesystem;
use weaver_core::CoreTypesPlugin;
use weaver_diagnostics::frame_time::{FrameTime, LogFrameTimePlugin};
use weaver_egui::{egui, EguiContext, EguiPlugin};
use weaver_gizmos::GizmoNodeLabel;
use weaver_q3::{
    bsp::{
        loader::{Bsp, BspLoader},
        render::BspRenderNodeLabel,
    },
    pk3::Pk3Filesystem,
    Q3Plugin,
};
use weaver_renderer::{
    camera::PrimaryCamera, clear_color::ClearColorPlugin, graph::RenderGraphApp, RenderApp,
};

pub mod camera;
pub mod transform_gizmo;

#[derive(Resource, Default)]
struct FpsHistory {
    pub history: VecDeque<f32>,
    pub display_fps: f32,
    smoothing_buffer: Vec<f32>,
}

fn main() -> Result<()> {
    env_logger::init();

    App::new()
        .add_plugin(CoreTypesPlugin)?
        .add_plugin(WinitPlugin {
            initial_size: (1920, 1080),
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
        .add_plugin(ClearColorPlugin(Color::new(0.1, 0.1, 0.1, 1.0)))?
        .configure_sub_app::<RenderApp>(|app| {
            app.add_render_main_graph_edge(SkyboxNodeLabel, BspRenderNodeLabel);
            app.add_render_main_graph_edge(BspRenderNodeLabel, GizmoNodeLabel);
        })
        .insert_resource(Skybox::new("assets/skyboxes/meadow_2k.hdr"))
        .insert_resource(Arc::new(
            Filesystem::default().with_pk3s_from_dir("assets/q3")?,
        ))
        .init_resource::<FpsHistory>()
        .add_system(setup, Init)
        .add_system(camera::update_camera, Update)
        .add_system(camera::update_aspect_ratio, Update)
        .add_system(fps_ui, Update)
        .run()
}

fn setup(
    mut commands: Commands,
    fs: Res<Arc<Filesystem>>,
    bsp_loader: Res<AssetLoadQueue<Bsp, BspLoader, PathAndFilesystem>>,
) {
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

    let bsp = bsp_loader.enqueue(("maps/q3dm6.bsp".into(), fs.clone()));
    commands.insert_resource(bsp);
}

fn fps_ui(
    time: Res<Time>,
    frame_time: Res<FrameTime>,
    mut history: ResMut<FpsHistory>,
    egui_ctx: Res<EguiContext>,
) {
    egui_ctx.with_ctx(|ctx| {
        egui::Window::new("Frame Time")
            .default_height(200.0)
            .show(ctx, |ui| {
                history.smoothing_buffer.push(frame_time.frame_time);

                if time.total_time > 1.0 && history.smoothing_buffer.len() > 100 {
                    let smoothed_fps = history.smoothing_buffer.iter().copied().sum::<f32>()
                        / history.smoothing_buffer.len() as f32;

                    history.smoothing_buffer.clear();

                    history.history.push_back(smoothed_fps);
                    if history.history.len() > 100 {
                        history.history.pop_front();
                    }

                    history.display_fps = smoothed_fps;
                }

                ui.label(format!("Frame Time: {:.4}ms", history.display_fps * 1000.0));
                ui.separator();

                let plot = egui_plot::Plot::new("Frame Time (ms)");
                let points = history
                    .history
                    .iter()
                    .enumerate()
                    .map(|(i, &fps)| [i as f64, fps as f64 * 1000.0])
                    .collect::<Vec<_>>();
                plot.show(ui, |plot| {
                    plot.line(egui_plot::Line::new(points).color(egui::Color32::LIGHT_GREEN));
                });
            });
    });
}
