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

    struct FooBarSystem;

    impl System for FooBarSystem {
        fn run(&self, world: &World) {
            let foobars = world.query::<Query<(Read<FooComponent>, Read<BarComponent>)>>();
            for (foo, bar) in foobars.iter() {
                println!("Foo: {}, Bar: {}", foo.0, bar.0);
            }
        }
    }

    #[system(Foo)]
    fn foo_system(foos: Query<Read<FooComponent>>) {
        for foo in foos.iter() {
            println!("Foo: {}", foo.0);
        }
    }

    #[system(Bar)]
    fn bar_system(bars: Query<Write<BarComponent>>) {
        for mut bar in bars.iter() {
            bar.0 += 1.0;
            println!("Bar: {}", bar.0);
        }
    }
}
