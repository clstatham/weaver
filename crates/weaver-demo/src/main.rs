use weaver::{prelude::*, weaver_app::App, weaver_renderer::camera::Camera};
use weaver_asset::AssetCommands;
use weaver_core::CoreTypesPlugin;
use weaver_diagnostics::frame_time::LogFrameTimePlugin;
use weaver_raytracing::RaytracingPlugin;
use weaver_renderer::{camera::PrimaryCamera, clear_color::ClearColorPlugin};
use weaver_winit::WindowPlugin;

pub mod camera;
pub mod transform_gizmo;

#[weaver::main]
async fn main() -> Result<()> {
    App::new()
        .add_plugin(CoreTypesPlugin)?
        .add_plugin(WindowPlugin::default())?
        .add_plugin(WinitPlugin)?
        .add_plugin(TimePlugin)?
        .add_plugin(InputPlugin)?
        .add_plugin(RendererPlugin)?
        .add_plugin(ClearColorPlugin(Color::new(0.1, 0.1, 0.1, 1.0)))?
        .configure_plugin::<WindowPlugin>(|plugin| {
            plugin.initial_size = (800, 600);
        })
        .add_plugin(RaytracingPlugin)?
        .add_plugin(LogFrameTimePlugin {
            log_interval: std::time::Duration::from_secs(5),
        })?
        .add_system(setup, Init)
        .add_system(camera::update_camera, Update)
        .add_system(camera::update_aspect_ratio, Update)
        .run()
}

async fn setup(commands: Commands) {
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
            }
            .looking_at(Vec3::new(100.0, 100.0, 100.0), Vec3::ZERO, Vec3::Y),
            PrimaryCamera,
        ))
        .await;

    let light_handle = commands
        .load_asset_direct(weaver_raytracing::material::Material {
            albedo: Color::WHITE,
            emission: Color::WHITE * 10.0,
        })
        .await;

    commands
        .spawn((
            Transform::from_translation(Vec3::new(0.0, 100.0, 0.0)),
            light_handle,
            weaver_raytracing::geometry::Sphere { radius: 20.0 },
        ))
        .await;

    let ground_handle = commands
        .load_asset_direct(weaver_raytracing::material::Material {
            albedo: Color::GREEN,
            emission: Color::BLACK,
        })
        .await;

    commands
        .spawn((
            Transform::from_translation(Vec3::new(0.0, -1000.0, 0.0)),
            ground_handle,
            weaver_raytracing::geometry::Sphere { radius: 1000.0 },
        ))
        .await;
}
