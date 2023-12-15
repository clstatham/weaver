use core::{mesh::Mesh, transform::Transform};

use app::App;
use ecs::world::World;

#[macro_use]
pub mod ecs;
pub mod app;
pub mod core;
pub mod renderer;

fn test_system(world: &mut World, delta: std::time::Duration) {
    for (_mesh, transform) in world.write::<Model>() {
        transform.rotate(1.0 * delta.as_secs_f32(), glam::Vec3::Y);
    }
}

type Model = (Mesh, Transform);

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut app = App::new(800, 600);

    let entity = app.spawn((Mesh::load_obj("assets/suzanne.obj")?, Transform::default()));

    app.register_system(test_system);

    app.run();

    Ok(())
}
