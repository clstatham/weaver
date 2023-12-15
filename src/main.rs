use app::App;
use ecs::world::World;

#[macro_use]
pub mod ecs;
pub mod app;

fn test_system(world: &mut World, delta: std::time::Duration) {
    let query = world.write::<(i32, &str)>();
    for (i, s) in query {
        *i += 1;
        println!("{}: {}", i, s);
    }
    println!("delta: {:?}", delta);
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut app = App::new();

    let entity = app.spawn((42, "Hello, world!"));
    app.add_component(entity, 69);

    app.register_system(test_system);

    app.run();

    Ok(())
}
