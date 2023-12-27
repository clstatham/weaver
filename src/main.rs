use app::App;

pub mod app;
pub mod core;
pub mod renderer;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut app = App::new(1600, 900);

    let model1 = app.load_model("assets/wooden_monkey.glb")?;

    app.run();

    Ok(())
}

#[cfg(test)]
mod tests {
    use weaver_ecs::{
        query::{Query, Queryable, Write},
        system, Bundle, Component, Read, System, World,
    };

    struct FooComponent(pub i32);

    unsafe impl Component for FooComponent {
        fn component_id() -> u64
        where
            Self: Sized,
        {
            0
        }
    }

    struct BarComponent(pub f32);

    unsafe impl Component for BarComponent {
        fn component_id() -> u64
        where
            Self: Sized,
        {
            1
        }
    }

    #[system(Foo)]
    fn foo_system(foo: Query<Read<FooComponent>>) {
        for foo in foo.iter() {
            println!("foo: {}", foo.0);
        }
    }

    #[system(Bar)]
    fn bar_system(bar: Query<Read<BarComponent>>) {
        for bar in bar.iter() {
            println!("bar: {}", bar.0);
        }
    }

    #[system(FooBar)]
    fn foobar_system(foo: Query<(Read<FooComponent>, Write<BarComponent>)>) {
        for (foo, bar) in foo.iter() {
            bar.0 += 1.0;
            println!("foobar: {} {}", foo.0, bar.0);
        }
    }

    #[test]
    fn test() {
        let mut world = weaver_ecs::World::new();
        world.add_system(Foo);
        world.add_system(Bar);
        world.add_system(FooBar);

        let entity = world.create_entity();
        world.add_component(entity, FooComponent(42));
        world.add_component(entity, BarComponent(6.9));

        world.update();
        world.update();
        world.update();
    }
}
