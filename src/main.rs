use app::App;

pub mod app;
pub mod core;
pub mod renderer;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut app = App::builder(1600, 900).build();

    let model1 = app.load_model("assets/wooden_monkey.glb")?;

    app.run();

    Ok(())
}

#[cfg(test)]
mod tests {
    use weaver_ecs::{system, Bundle, Component, Read, Write};

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

    #[system(BarSys)]
    fn bar_system(foo: Read<FooComponent>, mut bar: Write<BarComponent>) {
        for foo in foo.iter() {
            println!("bar_system foo={}", foo.0);
        }
        for bar in bar.iter_mut() {
            bar.0 += 1.0;
            println!("bar_system bar={}", bar.0);
        }
    }

    #[test]
    fn test_bundle() {
        #[derive(Bundle)]
        struct FooBarBundle {
            foo: FooComponent,
            bar: BarComponent,
        }

        let mut world = weaver_ecs::World::new();
        let entity = world.build(FooBarBundle {
            foo: FooComponent(42),
            bar: BarComponent(4.2069),
        });
        let entity2 = world.create_entity();
        world.add_component(entity2, BarComponent(69.420));

        world.add_system(BarSys);

        world.update();
        world.update();
        world.update();

        let foo = world.read::<FooComponent>();
        let bar = world.read::<BarComponent>();

        assert_eq!(foo.get(entity).unwrap().0, 42);
        assert_eq!(bar.get(entity).unwrap().0, 7.2069);
        assert_eq!(bar.get(entity2).unwrap().0, 72.420);
    }
}
