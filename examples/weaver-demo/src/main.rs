use weaver::{DefaultPlugins, prelude::*};
use weaver_diagnostics::frame_time::LogFrameTimePlugin;

pub mod camera;

fn main() -> Result<()> {
    App::new()
        .add_plugin(DefaultPlugins::new().with_window_size(1600, 900))?
        .add_plugin(LogFrameTimePlugin::default())?
        .insert_resource(Skybox::new("assets/skyboxes/meadow_2k.hdr"))
        .add_system(setup, Init)
        .add_system(camera::update_camera, Update)
        .add_system(camera::update_aspect_ratio, Update)
        .run()
}

fn setup(
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
            sensitivity: 0.1,
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
        .unwrap();

    let LoadedMaterialMeshPrimitive { material, mesh, .. } = material_mesh.primitives.remove(0);

    let mesh = meshes.insert(mesh);
    let material = materials.insert(material);

    let size = 100.0f32;
    for x in 0..10 {
        for y in 0..10 {
            let transform =
                Transform::from_translation(Vec3::new(x as f32 * size, 0.0, y as f32 * size));
            commands.spawn((mesh, material, transform));
        }
    }
}
