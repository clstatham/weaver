use weaver::{
    app::App,
    ecs::{scene::Scene, system::SystemStage},
    prelude::Vec3,
    renderer::{camera::Camera, RendererPlugin},
    winit::WinitPlugin,
};

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let mut app = App::new()?;
    app.add_plugin(WinitPlugin {
        initial_size: (1600, 900),
    })?;
    app.add_plugin(RendererPlugin)?;

    app.add_system(setup, SystemStage::Init)?;

    app.run()
}

fn setup(scene: &Scene) -> anyhow::Result<()> {
    scene.create_node_with(Camera::perspective_lookat(
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::ZERO,
        Vec3::Y,
        45.0,
        1600.0 / 900.0,
        0.1,
        100.0,
    ));

    Ok(())
}
