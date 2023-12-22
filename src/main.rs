use core::{color::Color, input::Input, mesh::Mesh, transform::Transform};

use app::App;
use ecs::{component::Component, query::Without, world::World};

#[macro_use]
pub mod ecs;
pub mod app;
pub mod core;
pub mod renderer;

fn test_system(world: &mut World, delta: std::time::Duration) {
    let mouse_delta = world.read_resource::<Input>().unwrap().mouse_delta();
    for transform in world
        .write::<(Mesh, Transform, Without<Mark>)>()
        .get_mut::<Transform>()
    {
        transform.rotate(mouse_delta.x * delta.as_secs_f32(), glam::Vec3::Y);
    }
}

struct Mark;
impl Component for Mark {}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut app = App::new(800, 600);

    let mut mesh = Mesh::load_obj("assets/suzanne.obj")?;

    for vertex in mesh.vertices.iter_mut() {
        vertex.color = Color::new(1.0, 0.0, 0.0);
    }
    let monkey1 = app.build((
        mesh.clone(),
        Transform::default().translate(-1.0, 0.0, 0.0),
        Mark,
    ));

    for vertex in mesh.vertices.iter_mut() {
        vertex.color = Color::new(0.0, 1.0, 0.0);
    }
    let monkey2 = app.build((mesh.clone(), Transform::default().translate(1.0, 0.0, 0.0)));

    for vertex in mesh.vertices.iter_mut() {
        vertex.color = Color::new(0.0, 0.0, 1.0);
    }
    let monkey3 = app.build((mesh.clone(), Transform::default().translate(0.0, 1.0, 0.0)));

    for vertex in mesh.vertices.iter_mut() {
        vertex.color = Color::new(1.0, 1.0, 0.0);
    }
    let monkey4 = app.build((
        mesh.clone(),
        Transform::default().translate(0.0, -1.0, 0.0),
        Mark,
    ));

    app.register_system(test_system);

    app.run();

    Ok(())
}
