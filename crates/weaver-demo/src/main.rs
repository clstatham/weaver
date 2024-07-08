use camera::CameraUpdate;
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
use weaver_diagnostics::frame_time::LogFrameTimePlugin;
use weaver_gizmos::GizmoNodeLabel;
use weaver_q3::{
    bsp::{
        loader::{Bsp, BspLoader},
        render::BspRenderNodeLabel,
    },
    pk3::Pk3Filesystem,
    shader::loader::LexedShaderCache,
    Q3Plugin,
};
use weaver_renderer::{
    camera::PrimaryCamera, clear_color::ClearColorPlugin, graph::RenderGraphApp, RenderApp,
};

pub mod camera;
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
pub enum VisMode {
    #[default]
    None,
    Nodes,
    Leaves,
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
        .insert_resource(Filesystem::default().with_pk3s_from_dir("assets/q3")?)
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
    mut commands: Commands,
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

    let bsp = bsp_loader.load_from_filesystem(&mut fs, "maps/pro-q3dm6.bsp")?;
    commands.insert_resource(bsp);

    Ok(())
}
