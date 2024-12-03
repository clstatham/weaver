use std::{path::PathBuf, str::FromStr, sync::Arc};

use weaver::{prelude::*, weaver_app::App, weaver_renderer::camera::Camera};
use weaver_asset::{AssetCommands, Filesystem};
use weaver_diagnostics::frame_time::LogFrameTimePlugin;
use weaver_q3::{
    bsp::{loader::BspLoader, render::render_bsps},
    pk3::Pk3Filesystem,
    Q3Plugin,
};
use weaver_renderer::{camera::PrimaryCamera, RenderApp, RenderStage};
use weaver_winit::WindowPlugin;

pub mod camera;
pub mod transform_gizmo;

// #[derive(Default)]
// struct FpsHistory {
//     pub history: VecDeque<f32>,
//     pub display_fps: f32,
//     smoothing_buffer: Vec<f32>,
// }

#[weaver::main]
async fn main() -> Result<()> {
    App::new()
        .add_plugin(DefaultPlugins)?
        .configure_plugin::<WindowPlugin>(|plugin| {
            plugin.initial_size = (1920, 1080);
        })
        // .add_plugin(GizmoPlugin)?
        // .add_plugin(EguiPlugin)?
        .add_plugin(Q3Plugin)?
        .add_plugin(LogFrameTimePlugin {
            log_interval: std::time::Duration::from_secs(5),
        })?
        .configure_sub_app::<RenderApp>(|app| {
            app.world_mut()
                .order_systems(render_skybox, render_bsps, RenderStage::Render);
        })
        .insert_resource(Skybox::new("assets/skyboxes/meadow_2k.hdr"))
        .insert_resource(Arc::new(
            Filesystem::default().with_pk3s_from_dir("assets/q3")?,
        ))
        // .init_resource::<FpsHistory>()
        .add_system(setup, Init)
        .add_system(camera::update_camera, Update)
        .add_system(camera::update_aspect_ratio, Update)
        // .add_system(fps_ui, Update)
        .run()
}
async fn setup(commands: Commands, fs: Res<Arc<Filesystem>>) {
    commands
        .spawn((
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
        ))
        .await;

    let bsp = commands
        .load_asset::<_, BspLoader, _>((PathBuf::from_str("maps/q3dm6.bsp").unwrap(), fs.clone()))
        .await;

    commands.insert_resource(bsp).await;
}

// async fn fps_ui(
//     time: Res<Time>,
//     frame_time: Res<FrameTime>,
//     mut history: ResMut<FpsHistory>,
//     egui_ctx: Res<EguiContext>,
// ) {
//     egui_ctx.with_ctx(|ctx| {
//         egui::Window::new("Frame Time")
//             .default_height(200.0)
//             .show(ctx, |ui| {
//                 history.smoothing_buffer.push(frame_time.frame_time);

//                 if time.total_time > 1.0 && history.smoothing_buffer.len() > 100 {
//                     let smoothed_fps = history.smoothing_buffer.iter().copied().sum::<f32>()
//                         / history.smoothing_buffer.len() as f32;

//                     history.smoothing_buffer.clear();

//                     history.history.push_back(smoothed_fps);
//                     if history.history.len() > 100 {
//                         history.history.pop_front();
//                     }

//                     history.display_fps = smoothed_fps;
//                 }

//                 ui.label(format!("Frame Time: {:.4}ms", history.display_fps * 1000.0));
//                 ui.separator();

//                 let plot = egui_plot::Plot::new("Frame Time (ms)");
//                 let points = history
//                     .history
//                     .iter()
//                     .enumerate()
//                     .map(|(i, &fps)| [i as f64, fps as f64 * 1000.0])
//                     .collect::<Vec<_>>();
//                 plot.show(ui, |plot| {
//                     plot.line(egui_plot::Line::new(points).color(egui::Color32::LIGHT_GREEN));
//                 });
//             });
//     });
// }
