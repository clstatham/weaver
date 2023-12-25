use core::{
    color::Color,
    input::Input,
    light::{DirectionalLight, Light, PointLight, SpotLight},
    mesh::Mesh,
    transform::Transform,
};

use app::App;
use ecs::{component::Component, query::Without, world::World};

#[macro_use]
pub mod ecs;
pub mod app;
pub mod core;
pub mod renderer;

fn test_system(world: &mut World, delta: std::time::Duration) {
    let (w, s, a, d) = {
        let input = world.read_resource::<Input>().unwrap();
        (
            input.is_key_pressed(winit::event::VirtualKeyCode::W),
            input.is_key_pressed(winit::event::VirtualKeyCode::S),
            input.is_key_pressed(winit::event::VirtualKeyCode::A),
            input.is_key_pressed(winit::event::VirtualKeyCode::D),
        )
    };
    let delta = delta.as_secs_f32();
    for transform in world
        .write::<(Mesh, Transform, Mark)>()
        .get_mut::<Transform>()
    {
        transform.rotate(1.0 * delta, glam::Vec3::Y);
        // if w {
        //     transform.translate(-1.0 * delta, 0.0, -1.0 * delta);
        // }
        // if s {
        //     transform.translate(1.0 * delta, 0.0, 1.0 * delta);
        // }
        // if a {
        //     transform.translate(1.0 * delta, 0.0, -1.0 * delta);
        // }
        // if d {
        //     transform.translate(-1.0 * delta, 0.0, 1.0 * delta);
        // }
    }

    for light in world.write::<Light>().get_mut::<Light>() {
        if let Light::Spot(light) = light {
            if w {
                light.angle += 1.0 * delta;
                light.angle = light.angle.clamp(0.0, 90.0);
                dbg!(light.angle);
            }
            if s {
                light.angle -= 1.0 * delta;
                light.angle = light.angle.clamp(0.0, 90.0);
                dbg!(light.angle);
            }
        }
    }
}

struct Mark;
impl Component for Mark {}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut app = App::new(800, 600);

    // add lights
    app.spawn(Light::Directional(DirectionalLight::new(
        glam::Vec3::new(1.0, 0.0, 0.0).normalize(),
        Color::new(1.0, 1.0, 1.0),
        1.0,
    )));
    app.spawn(Light::Spot(SpotLight::new(
        glam::Vec3::new(-10.0, 0.0, 0.0),
        glam::Vec3::new(1.0, 0.0, 0.0).normalize(),
        Color::new(1.0, 0.0, 0.0),
        1.0,
        30.0,
    )));

    let mesh = Mesh::load_gltf("assets/wooden_monkey.glb")?;

    // for vertex in mesh.vertices.iter_mut() {
    //     vertex.color = Color::new(1.0, 0.0, 0.0);
    // }
    app.build((
        mesh.clone(),
        Transform::default().translate(0.0, 0.0, 0.0),
        Mark,
    ));

    // for vertex in mesh.vertices.iter_mut() {
    //     vertex.color = Color::new(0.0, 1.0, 0.0);
    // }
    // let monkey2 = app.build((mesh.clone(), Transform::default().translate(1.0, 0.0, 0.0)));

    // for vertex in mesh.vertices.iter_mut() {
    //     vertex.color = Color::new(0.0, 0.0, 1.0);
    // }
    // let monkey3 = app.build((mesh.clone(), Transform::default().translate(0.0, 1.0, 0.0)));

    // for vertex in mesh.vertices.iter_mut() {
    //     vertex.color = Color::new(1.0, 1.0, 0.0);
    // }
    // let monkey4 = app.build((
    //     mesh.clone(),
    //     Transform::default().translate(0.0, -1.0, 0.0),
    //     Mark,
    // ));

    app.register_system(test_system);

    app.run();

    Ok(())
}
