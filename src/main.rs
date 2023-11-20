use ecs::{
    component::{Component, Field},
    system::{Query, ResolvedQuery, System, SystemLogic},
};

pub mod app;
#[macro_use]
pub mod ecs;
pub mod renderer;

fn test_system(queries: &[ResolvedQuery]) {
    dbg!(queries);
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let mut app = app::App::new((800, 600), "Weaver");

    let ent1 = app.world.create_entity();
    let ent2 = app.world.create_entity();
    let mut health = Component::new("health".to_string());
    health.add_field("value", Field::U32(100));
    app.world.add_component(ent1, health);
    let mut health = Component::new("health".to_string());
    health.add_field("value", Field::U32(69));
    app.world.add_component(ent2, health);
    let mut test = System::new("test_system".to_string(), SystemLogic::Static(test_system));
    test.queries.push(Query::Immutable("health".to_string()));
    app.world.add_system(test);

    app.run()
}
