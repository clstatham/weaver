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
    use weaver_ecs::{system, Bundle, Component, Query, Queryable, Read, System, World, Write};

    struct CompA(pub i32);

    unsafe impl Component for CompA {
        fn component_id() -> u64
        where
            Self: Sized,
        {
            0
        }
    }

    struct CompB(pub f32);

    unsafe impl Component for CompB {
        fn component_id() -> u64
        where
            Self: Sized,
        {
            1
        }
    }

    struct CompC(pub bool);

    unsafe impl Component for CompC {
        fn component_id() -> u64
        where
            Self: Sized,
        {
            2
        }
    }

    struct CompD(pub String);

    unsafe impl Component for CompD {
        fn component_id() -> u64
        where
            Self: Sized,
        {
            3
        }
    }

    #[derive(Bundle)]
    struct BundleABCD {
        a: CompA,
        b: CompB,
        c: CompC,
        d: CompD,
    }

    #[test]
    fn test_4_systems() {
        #[system(A)]
        fn system_a(a: Query<Read<CompA>>) {
            for a in a.iter() {
                println!("A: {}", a.0);
            }
        }
        #[system(B)]
        fn system_b(b: Query<Read<CompB>>) {
            for b in b.iter() {
                println!("B: {}", b.0);
            }
        }
        #[system(C)]
        fn system_c(c: Query<Read<CompC>>) {
            for c in c.iter() {
                println!("C: {}", c.0);
            }
        }
        #[system(D)]
        fn system_d(d: Query<Read<CompD>>) {
            for d in d.iter() {
                println!("D: {}", d.0);
            }
        }

        let mut world = World::new();
        world.spawn(BundleABCD {
            a: CompA(1),
            b: CompB(2.0),
            c: CompC(true),
            d: CompD("hello".to_string()),
        });

        world.add_system(A);
        world.add_system(B);
        world.add_system(C);
        world.add_system(D);

        world.update();
        world.update();
        world.update();
    }

    #[test]
    fn test_4_queries() {
        #[system(A)]
        fn system_a(
            a: Query<Read<CompA>>,
            b: Query<Read<CompB>>,
            c: Query<Read<CompC>>,
            d: Query<Read<CompD>>,
        ) {
            for a in a.iter() {
                println!("A: {}", a.0);
            }
            for b in b.iter() {
                println!("B: {}", b.0);
            }
            for c in c.iter() {
                println!("C: {}", c.0);
            }
            for d in d.iter() {
                println!("D: {}", d.0);
            }
        }

        let mut world = World::new();
        world.spawn(BundleABCD {
            a: CompA(1),
            b: CompB(2.0),
            c: CompC(true),
            d: CompD("hello".to_string()),
        });

        world.add_system(A);

        world.update();
        world.update();
        world.update();
    }

    #[test]
    fn test_one_big_query() {
        #[system(A)]
        fn system_a(q: Query<(Read<CompA>, Read<CompB>, Read<CompC>, Read<CompD>)>) {
            for (a, b, c, d) in q.iter() {
                println!("A: {}", a.0);
                println!("B: {}", b.0);
                println!("C: {}", c.0);
                println!("D: {}", d.0);
            }
        }

        let mut world = World::new();
        world.spawn(BundleABCD {
            a: CompA(1),
            b: CompB(2.0),
            c: CompC(true),
            d: CompD("hello".to_string()),
        });

        world.add_system(A);

        world.update();
        world.update();
        world.update();
    }

    #[test]
    fn test_4_systems_with_writes() {
        #[system(A)]
        fn system_a(mut a: Query<Write<CompA>>) {
            for mut a in a.iter() {
                println!("A: {}", a.0);
                a.0 += 1;
            }
        }
        #[system(B)]
        fn system_b(mut b: Query<Write<CompB>>) {
            for mut b in b.iter() {
                println!("B: {}", b.0);
                b.0 += 1.0;
            }
        }
        #[system(C)]
        fn system_c(mut c: Query<Write<CompC>>) {
            for mut c in c.iter() {
                println!("C: {}", c.0);
                c.0 = !c.0;
            }
        }
        #[system(D)]
        fn system_d(mut d: Query<Write<CompD>>) {
            for mut d in d.iter() {
                println!("D: {}", d.0);
                d.0 = "world".to_string();
            }
        }

        let mut world = World::new();
        world.spawn(BundleABCD {
            a: CompA(1),
            b: CompB(2.0),
            c: CompC(true),
            d: CompD("hello".to_string()),
        });

        world.add_system(A);
        world.add_system(B);
        world.add_system(C);
        world.add_system(D);

        world.update();
        world.update();
        world.update();
    }

    #[test]
    fn test_4_queries_with_writes() {
        #[system(A)]
        fn system_a(
            mut a: Query<Write<CompA>>,
            mut b: Query<Write<CompB>>,
            mut c: Query<Write<CompC>>,
            mut d: Query<Write<CompD>>,
        ) {
            for mut a in a.iter() {
                println!("A: {}", a.0);
                a.0 += 1;
            }
            for mut b in b.iter() {
                println!("B: {}", b.0);
                b.0 += 1.0;
            }
            for mut c in c.iter() {
                println!("C: {}", c.0);
                c.0 = !c.0;
            }
            for mut d in d.iter() {
                println!("D: {}", d.0);
                d.0 = "world".to_string();
            }
        }

        let mut world = World::new();
        world.spawn(BundleABCD {
            a: CompA(1),
            b: CompB(2.0),
            c: CompC(true),
            d: CompD("hello".to_string()),
        });

        world.add_system(A);

        world.update();
        world.update();
        world.update();
    }

    #[test]
    fn test_one_big_query_with_writes() {
        #[system(A)]
        fn system_a(mut q: Query<(Write<CompA>, Write<CompB>, Write<CompC>, Write<CompD>)>) {
            for (mut a, mut b, mut c, mut d) in q.iter() {
                println!("A: {}", a.0);
                println!("B: {}", b.0);
                println!("C: {}", c.0);
                println!("D: {}", d.0);
                a.0 += 1;
                b.0 += 1.0;
                c.0 = !c.0;
                d.0 = "world".to_string();
            }
        }

        let mut world = World::new();
        world.spawn(BundleABCD {
            a: CompA(1),
            b: CompB(2.0),
            c: CompC(true),
            d: CompD("hello".to_string()),
        });

        world.add_system(A);

        world.update();
        world.update();
        world.update();
    }

    #[test]
    #[should_panic]
    fn test_conflicting_writes() {
        #[system(A)]
        fn system_a(mut a: Query<Write<CompA>>, mut b: Query<Write<CompA>>) {
            for mut a in a.iter() {
                println!("A: {}", a.0);
                a.0 += 1;
            }
            for mut b in b.iter() {
                println!("B: {}", b.0);
                b.0 += 1;
            }
        }

        let mut world = World::new();
        world.spawn(BundleABCD {
            a: CompA(1),
            b: CompB(2.0),
            c: CompC(true),
            d: CompD("hello".to_string()),
        });

        world.add_system(A);

        world.update();
        world.update();
        world.update();
    }

    #[test]
    #[should_panic]
    fn test_conflicting_writes_with_reads() {
        #[system(A)]
        fn system_a(mut a: Query<Write<CompA>>, c: Query<Read<CompA>>) {
            for mut a in a.iter() {
                println!("A: {}", a.0);
                a.0 += 1;
            }
            for c in c.iter() {
                println!("C: {}", c.0);
            }
        }

        let mut world = World::new();
        world.spawn(BundleABCD {
            a: CompA(1),
            b: CompB(2.0),
            c: CompC(true),
            d: CompD("hello".to_string()),
        });

        world.add_system(A);

        world.update();
        world.update();
        world.update();
    }
}
