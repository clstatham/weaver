use std::path::PathBuf;

use weaver::prelude::*;
use weaver_diagnostics::prelude::*;

pub mod camera;
pub mod transform_gizmo;

fn main() -> Result<()> {
    App::new()
        .add_plugin(CoreTypesPlugin)?
        .add_plugin(WindowPlugin::default())?
        .add_plugin(WinitPlugin)?
        .add_plugin(TimePlugin)?
        .add_plugin(InputPlugin)?
        .add_plugin(RendererPlugin)?
        .add_plugin(ClearColorPlugin(Color::new(0.1, 0.1, 0.1, 1.0)))?
        .configure_plugin::<WindowPlugin>(|plugin| {
            plugin.initial_size = (1600, 900);
        })
        .add_plugin(PbrPlugin)?
        .add_plugin(LogFrameTimePlugin {
            log_interval: std::time::Duration::from_secs(1),
        })?
        .insert_resource(Skybox::new("assets/skyboxes/meadow_2k.hdr"))
        .add_system(setup, Init)
        .add_system(camera::update_camera, Update)
        .add_system(camera::update_aspect_ratio, Update)
        .run()
}

async fn setup(
    commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Material>>,
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
        }
        .looking_at(Vec3::new(100.0, 100.0, 100.0), Vec3::ZERO, Vec3::Y),
        PrimaryCamera,
    ));

    commands.spawn((
        Transform::from_translation(Vec3::new(100.0, 100.0, 100.0)),
        PointLight {
            color: Color::WHITE,
            intensity: 1000.0,
            radius: 1000.0,
            enabled: true,
        },
    ));

    let mut material_mesh = GltfMaterialModelLoader
        .load(
            PathBuf::from("assets/meshes/stanford_dragon_pbr.glb"),
            &commands,
        )
        .await
        .unwrap();

    let LoadedMaterialMeshPrimitive { material, mesh, .. } = material_mesh.primitives.remove(0);

    let mesh = meshes.insert(mesh);
    let material = materials.insert(material);

    commands.spawn((mesh, material, Transform::default()));
}
