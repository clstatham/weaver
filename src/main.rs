use ecs::{
    component::{Component, Field},
    system::{Query, ResolvedQuery, System, SystemLogic},
};
use renderer::{color::Color, obj_loader::load_obj};

pub mod app;
#[macro_use]
pub mod ecs;
pub mod editor;
pub mod graph;
pub mod gui;
pub mod renderer;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let mut app = app::App::new((800, 600), "Weaver");

    app.renderer.camera.position = glam::Vec3::new(1.0, 1.0, 1.0);
    app.renderer.lights.push(renderer::light::PointLight {
        position: glam::Vec3::new(0.0, 1.0, 0.0),
        color: Color::new(1.0, 1.0, 1.0),
        intensity: 1.0,
    });

    let e1 = app.world.create_entity();
    let mut mesh1 = Component::new("mesh".to_string());

    // // Vertices of a cube.
    // #[rustfmt::skip]
    // let vertices = vec![
    //     Vertex {
    //         position: glam::Vec3::new(-1.0, -1.0, -1.0),
    //         color: Color::new(1.0, 0.0, 0.0),
    //         normal: None,
    //     },
    //     Vertex {
    //         position: glam::Vec3::new(1.0, -1.0, -1.0),
    //         color: Color::new(0.0, 1.0, 0.0),
    //         normal: None,
    //     },
    //     Vertex {
    //         position: glam::Vec3::new(-1.0, 1.0, -1.0),
    //         color: Color::new(0.0, 0.0, 1.0),
    //         normal: None,
    //     },
    //     Vertex {
    //         position: glam::Vec3::new(1.0, 1.0, -1.0),
    //         color: Color::new(1.0, 1.0, 0.0),
    //         normal: None,
    //     },
    //     Vertex {
    //         position: glam::Vec3::new(-1.0, -1.0, 1.0),
    //         color: Color::new(1.0, 0.0, 1.0),
    //         normal: None,
    //     },
    //     Vertex {
    //         position: glam::Vec3::new(1.0, -1.0, 1.0),
    //         color: Color::new(0.0, 1.0, 1.0),
    //         normal: None,
    //     },
    //     Vertex {
    //         position: glam::Vec3::new(-1.0, 1.0, 1.0),
    //         color: Color::new(1.0, 1.0, 1.0),
    //         normal: None,
    //     },
    //     Vertex {
    //         position: glam::Vec3::new(1.0, 1.0, 1.0),
    //         color: Color::new(1.0, 0.0, 0.0),
    //         normal: None,
    //     },
    // ];

    // // Indices of a cube with counter-clockwise normals.
    // #[rustfmt::skip]
    // let indices = vec![
    //     0, 1, 2, 2, 1, 3,
    //     1, 5, 3, 3, 5, 7,
    //     5, 4, 7, 7, 4, 6,
    //     4, 0, 6, 6, 0, 2,
    //     2, 3, 6, 6, 3, 7,
    //     4, 5, 0, 0, 5, 1,
    // ];

    let mut mesh = load_obj("assets/cube.obj").unwrap();
    for vert in mesh.vertices.iter_mut() {
        vert.color = Color::new(rand::random(), rand::random(), rand::random());
    }
    mesh1.add_field("mesh", Field::Mesh(mesh));
    app.world.add_component(e1, mesh1);
    let mut transform1 = Component::new("transform".to_string());
    transform1.add_field("position", Field::Vec3(glam::Vec3::new(0.0, 0.0, 0.0)));
    transform1.add_field("rotation", Field::Vec3(glam::Vec3::new(0.0, 0.0, 0.0)));
    transform1.add_field("scale", Field::Vec3(glam::Vec3::new(0.2, 0.2, 0.2)));
    app.world.add_component(e1, transform1);

    let scene = serde_yaml::to_string(&app.world)?;
    std::fs::write("assets/editor.yaml", scene)?;

    app.run()
}
