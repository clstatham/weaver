use raqote::{DrawOptions, DrawTarget, PathBuilder, SolidSource, StrokeStyle};
use rustc_hash::FxHashMap;

use crate::ecs::{
    component::Field,
    system::{Query, ResolvedQuery},
    world::World,
};

use self::camera::PerspectiveCamera;

pub mod camera;

pub fn render(
    draw_target: &mut DrawTarget,
    camera: &PerspectiveCamera,
    world: &World,
    screen_size: (u32, u32),
) -> anyhow::Result<()> {
    let query = Query::Immutable("mesh".to_string());
    let meshes = world.query(&query);
    let query = Query::Immutable("transform".to_string());
    let transforms = world.query(&query);
    let mut transformed_meshes = FxHashMap::default();

    if let (ResolvedQuery::Immutable(meshes), ResolvedQuery::Immutable(transforms)) =
        (meshes, transforms)
    {
        for mesh in meshes {
            let vertices = match mesh.fields.get("vertices") {
                Some(Field::List(vertices)) => vertices,
                _ => {
                    log::error!("mesh component does not have a vertices field");
                    continue;
                }
            };

            let mut transformed_vertices = Vec::new();
            for vertex in vertices {
                if let Field::Vec3(vertex) = vertex {
                    let transform = transforms
                        .iter()
                        .find(|transform| transform.entity == mesh.entity)
                        .unwrap();
                    let position = match transform.fields.get("position") {
                        Some(Field::Vec3(position)) => position,
                        _ => {
                            log::error!("transform component does not have a position field");
                            continue;
                        }
                    };
                    let rotation = match transform.fields.get("rotation") {
                        Some(Field::Vec3(rotation)) => rotation,
                        _ => {
                            log::error!("transform component does not have a rotation field");
                            continue;
                        }
                    };
                    let scale = match transform.fields.get("scale") {
                        Some(Field::Vec3(scale)) => scale,
                        _ => {
                            log::error!("transform component does not have a scale field");
                            continue;
                        }
                    };

                    // transform with the mesh's position, rotation, and scale
                    let rot_quat = glam::Quat::from_euler(
                        glam::EulerRot::XYZ,
                        rotation.x,
                        rotation.y,
                        rotation.z,
                    );
                    let transformed_vertex = rot_quat * *vertex;
                    let transformed_vertex = transformed_vertex.mul_add(*scale, *position);

                    // transform with the camera view and projection matrices
                    let transformed_vertex = camera
                        .get_view_projection_matrix()
                        .transform_point3(transformed_vertex);

                    transformed_vertices.push(transformed_vertex);
                }
            }

            transformed_meshes.insert(mesh.entity, transformed_vertices);
        }

        for (_entity, vertices) in transformed_meshes {
            let mut path_builder = PathBuilder::new();
            for (i, vertex) in vertices.iter().enumerate() {
                let vertex = vertex.mul_add(
                    glam::Vec3::new(screen_size.0 as f32, -(screen_size.1 as f32), 0.0),
                    glam::Vec3::new(screen_size.0 as f32 / 2.0, screen_size.1 as f32 / 2.0, 0.0),
                );
                if i == 0 {
                    path_builder.move_to(vertex.x, vertex.y);
                } else {
                    path_builder.line_to(vertex.x, vertex.y);
                }
            }
            let path = path_builder.finish();
            draw_target.fill(
                &path,
                &raqote::Source::Solid(SolidSource {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                }),
                &DrawOptions::new(),
            );
        }
    }

    Ok(())
}
