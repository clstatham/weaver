use app::App;
use weaver_proc_macro::system;

pub mod app;
pub mod core;
pub mod renderer;

// #[system]
// fn test_system(foo: i32, bar: f32) {
//     println!("test_system {foo} {bar}");
// }

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut app = App::builder(1600, 900).build();

    let model1 = app.load_model("assets/wooden_monkey.glb")?;

    app.run();

    Ok(())
}

#[cfg(test)]
mod tests {
    use weaver_ecs::{Component, Read};
    use weaver_proc_macro::system;

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

    #[system(Bar)]
    fn bar_system(foo: Read<FooComponent>, bar: Read<BarComponent>) {
        foo.iter().zip(bar.iter()).for_each(|(foo, bar)| {
            println!("bar_system {} {}", foo.0, bar.0);
        });
    }

    #[test]
    fn foo() {
        let mut world = weaver_ecs::World::new();
        let entity = world.create_entity();
        world.add_component(entity, FooComponent(42));
        world.add_component(entity, BarComponent(4.2069));
        world.add_system(Bar);

        world.update();
    }
}
