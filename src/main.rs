use ecs::world::World;

#[macro_use]
pub mod ecs;

fn test_system(world: &mut World) {
    let query = world.write::<(i32, &str)>();
    for (i, s) in query {
        *i += 1;
        println!("{}: {}", i, s);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut world = World::new();

    let e1 = world.spawn((42, "Hello, world!"));
    let e2 = world.spawn((1337, "Goodbye, world!"));

    world.register_system(test_system);

    world.update();
    world.update();
    world.update();
    world.update();

    Ok(())
}
