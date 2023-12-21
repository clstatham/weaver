use core::{mesh::Mesh, transform::Transform};

use app::App;
use ecs::world::World;

#[macro_use]
pub mod ecs;
pub mod app;
pub mod core;
pub mod renderer;

fn test_system(world: &mut World, delta: std::time::Duration) {
    for transform in world
        .write::<(Mesh, Transform, Mark)>()
        .get_mut::<Transform>()
    {
        transform.rotate(1.0 * delta.as_secs_f32(), glam::Vec3::Y);
    }
}

struct Mark;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut app = App::new(800, 600);

    // let _entity = app.build((Mesh::load_obj("assets/suzanne.obj")?, Transform::default()));

    let monkey1 = app.build((
        Mesh::load_obj("assets/suzanne.obj")?,
        Transform::default().translate(-1.0, 0.0, 0.0),
        Mark,
    ));
    let monkey2 = app.build((
        Mesh::load_obj("assets/suzanne.obj")?,
        Transform::default().translate(1.0, 0.0, 0.0),
    ));

    app.register_system(test_system);

    app.run();

    Ok(())
}
