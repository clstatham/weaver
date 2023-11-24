use rustc_hash::FxHashMap;

use crate::ecs::{
    component::Field,
    system::{Query, ResolvedQuery},
    world::World,
};

use self::camera::PerspectiveCamera;

pub mod camera;

/// Draws a line from (x0, y0) to (x1, y1) in the given frame.
pub fn line(frame: &mut [u32], frame_width: i32, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        let offset = x + y * frame_width;
        if offset < 0 || offset >= frame.len() as i32 {
            break;
        }
        frame[offset as usize] = color;
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Renders the given [World] to the given frame.
pub fn render(
    frame: &mut [u32],
    camera: &PerspectiveCamera,
    world: &World,
    screen_size: (u32, u32),
) -> anyhow::Result<()> {
    // Query the world for meshes and transforms.
    let query = Query::Immutable("mesh".to_string());
    let meshes = world.query(&query);
    let query = Query::Immutable("transform".to_string());
    let transforms = world.query(&query);
    let mut transformed_meshes = FxHashMap::default();

    if let (ResolvedQuery::Immutable(meshes), ResolvedQuery::Immutable(transforms)) =
        (meshes, transforms)
    {
        // Transform the meshes, storing the transformed meshes in a map.
        for mesh in meshes {
            let vertices = match mesh.fields.get("vertices") {
                Some(Field::List(vertices)) => vertices,
                _ => {
                    log::error!("mesh component does not have a vertices field");
                    continue;
                }
            };

            let mut transformed_mesh = Vec::new();
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

                    transformed_mesh.push(transformed_vertex);
                }
            }

            transformed_meshes.insert(mesh.entity, transformed_mesh);
        }

        // Rasterize lines from the transformed meshes.
        for (_entity, vertices) in transformed_meshes {
            let mut last = glam::Vec3::ZERO;
            for (i, vertex) in vertices.iter().enumerate() {
                let vertex = vertex.mul_add(
                    glam::Vec3::new(screen_size.0 as f32, -(screen_size.1 as f32), 0.0),
                    glam::Vec3::new(screen_size.0 as f32 / 2.0, screen_size.1 as f32 / 2.0, 0.0),
                );
                if i == 0 {
                    last = vertex;
                    continue;
                }

                line(
                    frame,
                    screen_size.0 as i32,
                    last.x as i32,
                    last.y as i32,
                    vertex.x as i32,
                    vertex.y as i32,
                    0xffffffff,
                );

                last = vertex;
            }
        }
    }

    Ok(())
}
