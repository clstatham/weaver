use ecs::{
    component::{Component, Field},
    system::{Query, ResolvedQuery, System, SystemLogic},
};

pub mod app;
#[macro_use]
pub mod ecs;
pub mod renderer;

fn test_system(queries: &mut [ResolvedQuery]) {
    // rotate any meshes
    let transforms = &mut queries[0];
    if let ResolvedQuery::Mutable(transforms) = transforms {
        for transform in transforms {
            let rotation = match transform.fields.get_mut("rotation") {
                Some(Field::Vec3(rotation)) => rotation,
                _ => {
                    log::error!("transform component does not have a rotation field");
                    continue;
                }
            };
            rotation.x += 0.00;
            rotation.y += 0.00;
            rotation.z += 0.01;
        }
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let mut app = app::App::new((800, 600), "Weaver");

    let e1 = app.world.create_entity();
    let mut mesh1 = Component::new("mesh".to_string());
    mesh1.add_field(
        "vertices",
        Field::List(vec![
            Field::Vec3(glam::Vec3::new(0.0, 0.0, 0.0)),
            Field::Vec3(glam::Vec3::new(0.1, 0.0, 0.0)),
            Field::Vec3(glam::Vec3::new(0.0, 0.1, 0.0)),
        ]),
    );
    app.world.add_component(e1, mesh1);
    let mut transform1 = Component::new("transform".to_string());
    transform1.add_field("position", Field::Vec3(glam::Vec3::new(0.0, 0.0, 1.0)));
    transform1.add_field("rotation", Field::Vec3(glam::Vec3::new(0.0, 0.0, 0.0)));
    transform1.add_field("scale", Field::Vec3(glam::Vec3::new(1.0, 1.0, 1.0)));
    app.world.add_component(e1, transform1);

    let mut s1 = System::new("test_system".to_string(), SystemLogic::Static(test_system));
    s1.add_query(Query::Mutable("transform".to_string()));
    app.world.add_system(s1);

    app.run()
}
