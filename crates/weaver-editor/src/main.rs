use weaver::{
    app::App,
    core::mesh::Mesh,
    ecs::{system::SystemStage, world::World},
    pbr::{camera::PbrCamera, material::Material, PbrPlugin},
    prelude::*,
    renderer::{camera::Camera, RendererPlugin},
    winit::WinitPlugin,
};

fn main() -> Result<()> {
    env_logger::init();
    let mut app = App::new()?;
    app.add_plugin(WinitPlugin {
        initial_size: (1600, 900),
    })?;
    app.add_plugin(AssetPlugin)?;
    app.add_plugin(RendererPlugin)?;
    app.add_plugin(PbrPlugin)?;

    app.add_system(setup, SystemStage::Init)?;
    app.add_system(update, SystemStage::Update)?;

    app.run()
}

fn setup(world: &World) -> Result<()> {
    let scene = world.root_scene();
    let camera = scene.create_node_with(Camera::perspective_lookat(
        Vec3::new(5.0, 5.0, 5.0),
        Vec3::ZERO,
        Vec3::Y,
        45.0,
        1600.0 / 900.0,
        0.1,
        100.0,
    ));
    world.insert_component(
        camera.entity(),
        PbrCamera::new(Color::new(0.1, 0.1, 0.1, 1.0)),
    );

    let asset_loader = world.get_resource::<AssetLoader>().unwrap();

    let mesh = asset_loader.load::<Mesh>("assets/meshes/cube.obj")?;
    let cube = scene.create_node_with(mesh);

    let material = asset_loader.load::<Material>("assets/materials/wood.glb")?;
    world.insert_component(cube.entity(), material);

    let transform = Transform::from_rotation(Quat::from_rotation_y(20.0f32.to_radians()));
    world.insert_component(cube.entity(), transform);

    Ok(())
}

fn update(world: &World) -> Result<()> {
    let query = world.query(&Query::new().read::<Transform>());

    for entity in query.iter() {
        let mut transform = world.get_component_mut::<Transform>(entity).unwrap();
        transform.rotation *= Quat::from_rotation_y(0.0001);
    }

    Ok(())
}
