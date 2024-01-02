use core::{
    camera::FlyCamera,
    input::Input,
    light::{DirectionalLight, PointLight},
    material::Material,
    mesh::Mesh,
    time::Time,
    transform::Transform,
};

use app::{asset_server::AssetServer, App};
use weaver_ecs::*;
use weaver_proc_macro::system;

pub mod app;
pub mod core;
pub mod renderer;

#[system(Update)]
fn update(
    model: Query<(Read<Mesh>, Write<Transform>, Read<Object>)>,
    lights: Query<Write<PointLight>>,
    timey: Res<Time>,
) {
    for (_i, (_mesh, mut transform, _)) in model.iter().enumerate() {
        transform.set_translation(glam::Vec3::new(
            5.0 * timey.total_time.sin(),
            5.0,
            5.0 * timey.total_time.cos(),
        ));
    }
    for mut light in lights.iter() {
        light.position.x = 5.0 * timey.total_time.sin();
        light.position.z = 5.0 * timey.total_time.cos();
    }
}

#[system(Spin)]
fn spin(model: Query<(Read<Mesh>, Write<Transform>, Read<Spinner>)>, timey: Res<Time>) {
    for (_i, (_mesh, mut transform, _)) in model.iter().enumerate() {
        transform.rotate(timey.delta_time * 1.75, glam::Vec3::Y);
    }
}

#[system(CameraUpdate)]
fn cammera_update(mut camera: ResMut<FlyCamera>, time: Res<Time>, input: Res<Input>) {
    camera.update(&input, time.delta_time);
}

#[derive(Component)]
struct Object;

#[derive(Component)]
struct Spinner;

#[allow(dead_code)]
enum Materials {
    Wood,
    Metal,
    WoodTile,
    BrickWall,
    StoneWall,
}

impl Materials {
    pub fn load(&self, asset_server: &mut AssetServer) -> anyhow::Result<Material> {
        match self {
            // Wood_025
            Materials::Wood => {
                let base_color = asset_server
                    .load_texture("assets/materials/Wood_025_SD/Wood_025_basecolor.jpg", false)?;
                let normal = asset_server
                    .load_texture("assets/materials/Wood_025_SD/Wood_025_normal.jpg", true)?;
                let roughness = asset_server
                    .load_texture("assets/materials/Wood_025_SD/Wood_025_roughness.jpg", false)?;
                let ao = asset_server.load_texture(
                    "assets/materials/Wood_025_SD/Wood_025_ambientOcclusion.jpg",
                    false,
                )?;
                Ok(Material::new(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                ))
            }
            // Metal_006
            Materials::Metal => {
                let base_color = asset_server.load_texture(
                    "assets/materials/Metal_006_SD/Metal_006_basecolor.jpg",
                    false,
                )?;
                let normal = asset_server
                    .load_texture("assets/materials/Metal_006_SD/Metal_006_normal.jpg", true)?;
                let roughness = asset_server.load_texture(
                    "assets/materials/Metal_006_SD/Metal_006_roughness.jpg",
                    false,
                )?;
                let ao = asset_server.load_texture(
                    "assets/materials/Metal_006_SD/Metal_006_ambientOcclusion.jpg",
                    false,
                )?;
                Ok(Material::new(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                ))
            }
            // Wood_Herringbone_Tiles_004
            Materials::WoodTile => {
                let base_color = asset_server.load_texture(
                    "assets/materials/Wood_Herringbone_Tiles_004_SD/Substance_Graph_BaseColor.jpg",
                    false,
                )?;
                let normal = asset_server.load_texture(
                    "assets/materials/Wood_Herringbone_Tiles_004_SD/Substance_Graph_Normal.jpg",
                    true,
                )?;
                let roughness = asset_server.load_texture(
                    "assets/materials/Wood_Herringbone_Tiles_004_SD/Substance_Graph_Roughness.jpg",
                    false,
                )?;
                let ao = asset_server.load_texture(
                    "assets/materials/Wood_Herringbone_Tiles_004_SD/Substance_Graph_AmbientOcclusion.jpg",
                    false,
                )?;
                Ok(Material::new(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                ))
            }
            // Brick_Wall_017
            Materials::BrickWall => {
                let base_color = asset_server.load_texture(
                    "assets/materials/Brick_Wall_017_SD/Brick_Wall_017_basecolor.jpg",
                    false,
                )?;
                let normal = asset_server.load_texture(
                    "assets/materials/Brick_Wall_017_SD/Brick_Wall_017_normal.jpg",
                    true,
                )?;
                let roughness = asset_server.load_texture(
                    "assets/materials/Brick_Wall_017_SD/Brick_Wall_017_roughness.jpg",
                    false,
                )?;
                let ao = asset_server.load_texture(
                    "assets/materials/Brick_Wall_017_SD/Brick_Wall_017_ambientOcclusion.jpg",
                    false,
                )?;
                Ok(Material::new(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                ))
            }
            // Wall_Stone_021
            Materials::StoneWall => {
                let base_color = asset_server.load_texture(
                    "assets/materials/Wall_Stone_021_SD/Substance_graph_BaseColor.jpg",
                    false,
                )?;
                let normal = asset_server.load_texture(
                    "assets/materials/Wall_Stone_021_SD/Substance_graph_Normal.jpg",
                    true,
                )?;
                let roughness = asset_server.load_texture(
                    "assets/materials/Wall_Stone_021_SD/Substance_graph_Roughness.jpg",
                    false,
                )?;
                let ao = asset_server.load_texture(
                    "assets/materials/Wall_Stone_021_SD/Substance_graph_AmbientOcclusion.jpg",
                    false,
                )?;
                Ok(Material::new(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                ))
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut app = App::new(1600, 900);

    // app.spawn(DirectionalLight::new(
    //     glam::Vec3::new(1.0, -1.0, 1.0).normalize(),
    //     core::color::Color::WHITE,
    //     40.0,
    // ));

    app.spawn(PointLight::new(
        glam::Vec3::new(5.0, 5.0, 5.0),
        core::color::Color::WHITE,
        20.0,
    ));

    // app.spawn(PointLight::new(
    //     glam::Vec3::new(0.0, 5.0, 0.0),
    //     core::color::Color::WHITE,
    //     20.0,
    // ));

    let room_scale = 30.0;

    app.build(|commands, asset_server| {
        // floor
        let mesh = asset_server.load_mesh("assets/woodcube.glb").unwrap();
        let mut material = Materials::WoodTile.load(asset_server).unwrap();
        material.texture_scaling = room_scale;
        commands.spawn((
            mesh,
            material,
            Transform::new()
                .scale(room_scale, 1.0, room_scale)
                .translate(0.0, -2.0, 0.0),
        ));

        // object circle
        let num_objects = 20;
        let radius = 10.0;

        for i in 0..num_objects {
            let angle = (i as f32 / num_objects as f32) * std::f32::consts::TAU;
            let x = angle.cos() * radius;
            let z = angle.sin() * radius;

            let mesh = asset_server
                .load_mesh("assets/woodmonkey_highpoly.glb")
                .unwrap();
            let material = Materials::Metal.load(asset_server).unwrap();
            commands.spawn((
                mesh,
                material,
                Transform::new().translate(x, 0.0, z),
                Spinner,
            ));
        }
    });

    app.add_system(CameraUpdate);
    app.add_system(Update);
    app.add_system(Spin);

    app.run();

    Ok(())
}

#[cfg(test)]
mod tests {
    use weaver_ecs::{system, Bundle, Component, Queryable, Read, World, Write};

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
}
