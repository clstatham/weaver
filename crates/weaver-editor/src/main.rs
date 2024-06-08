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
        45.0f32.to_radians(),
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

    let material = asset_loader.load::<Material>("assets/materials/wood_tiles.glb")?;
    {
        let mut assets = world.get_resource_mut::<Assets>().unwrap();
        assets.get_mut::<Material>(material).unwrap().texture_scale = 100.0;
    }
    world.insert_component(cube.entity(), material);

    let mut transform = Transform::from_rotation(Quat::from_rotation_y(20.0f32.to_radians()));
    transform.translation = Vec3::new(0.0, -1.0, 0.0);
    transform.scale = Vec3::new(10.0, 1.0, 10.0);
    world.insert_component(cube.entity(), transform);

    let _light1 = scene.create_node_with(PointLight {
        position: Vec3::new(10.0, 5.0, 10.0),
        color: Color::WHITE,
        intensity: 100.0,
        radius: 100.0,
    });

    let _light2 = scene.create_node_with(PointLight {
        position: Vec3::new(-10.0, 5.0, -10.0),
        color: Color::GREEN,
        intensity: 100.0,
        radius: 100.0,
    });

    Ok(())
}

fn update(world: &World) -> Result<()> {
    let query = world.query(&Query::new().read::<Transform>());

    for entity in query.iter() {
        let mut transform = world.get_component_mut::<Transform>(entity).unwrap();
        transform.rotation *= Quat::from_rotation_y(0.001);
    }

    Ok(())
}
