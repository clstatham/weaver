use std::path::Path;

use weaver_core::{
    mesh::{Mesh, Vertex},
    prelude::*,
};
use weaver_util::prelude::*;

pub fn load_obj(path: impl AsRef<Path>) -> Result<Mesh> {
    let path = path.as_ref();
    let (models, _) = tobj::load_obj(
        path,
        &tobj::LoadOptions {
            single_index: true,
            triangulate: true,
            ..Default::default()
        },
    )?;

    if models.len() != 1 {
        bail!("expected exactly one model in OBJ file: {:?}", path);
    }

    let mesh = &models[0].mesh;

    let mut vertices = Vec::with_capacity(mesh.positions.len() / 3);
    let mut indices = Vec::with_capacity(mesh.indices.len());

    for i in 0..mesh.positions.len() / 3 {
        let position = [
            mesh.positions[i * 3],
            mesh.positions[i * 3 + 1],
            mesh.positions[i * 3 + 2],
        ];
        let normal = [
            mesh.normals[i * 3],
            mesh.normals[i * 3 + 1],
            mesh.normals[i * 3 + 2],
        ];
        let uv = [mesh.texcoords[i * 2], mesh.texcoords[i * 2 + 1]];

        vertices.push(Vertex {
            position: Vec3::from(position),
            normal: Vec3::from(normal).normalize(),
            tex_coords: Vec2::from(uv),
            tangent: Vec3::ZERO,
        });
    }

    for index in &mesh.indices {
        indices.push(*index);
    }

    calculate_tangents(&mut vertices, &indices);

    Ok(Mesh::new(vertices, indices))
}

fn calculate_tangents(vertices: &mut [Vertex], indices: &[u32]) {
    for vertex in vertices.iter_mut() {
        vertex.tangent = Vec3::ZERO;
    }

    let mut num_triangles = vec![0; vertices.len()];
    for c in indices.chunks(3) {
        let i0 = c[0] as usize;
        let i1 = c[1] as usize;
        let i2 = c[2] as usize;

        let v0 = vertices[i0].position;
        let v1 = vertices[i1].position;
        let v2 = vertices[i2].position;

        let uv0 = vertices[i0].tex_coords;
        let uv1 = vertices[i1].tex_coords;
        let uv2 = vertices[i2].tex_coords;

        let delta_pos1 = v1 - v0;
        let delta_pos2 = v2 - v0;

        let delta_uv1 = uv1 - uv0;
        let delta_uv2 = uv2 - uv0;

        let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
        let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;

        vertices[i0].tangent += tangent;
        vertices[i1].tangent += tangent;
        vertices[i2].tangent += tangent;

        num_triangles[i0] += 1;
        num_triangles[i1] += 1;
        num_triangles[i2] += 1;
    }

    for (vertex, num_triangles) in vertices.iter_mut().zip(num_triangles) {
        vertex.tangent /= num_triangles as f32;

        // gram-schmidt orthogonalize
        let tangent = vertex.tangent - vertex.normal * vertex.normal.dot(vertex.tangent);
        vertex.tangent = tangent.normalize();

        // check for orthogonality
        let ndt = vertex.normal.dot(vertex.tangent);
        debug_assert!(
            ndt < 0.001,
            "normal and tangent are not orthogonal: N . T = {:?}",
            ndt
        );

        // sanity check with the binormal
        let binormal = vertex.normal.cross(vertex.tangent);
        let ndb = vertex.normal.dot(binormal);
        debug_assert!(
            ndb < 0.001,
            "normal and binormal are not orthogonal: N . B = {:?}",
            ndb
        );
        let bdt = binormal.dot(vertex.tangent);
        debug_assert!(
            bdt < 0.001,
            "binormal and tangent are not orthogonal: B . T = {:?}",
            bdt
        );

        // calculate handedness
        let tangent = if vertex.normal.cross(vertex.tangent).dot(vertex.tangent) < 0.0 {
            -vertex.tangent
        } else {
            vertex.tangent
        };
        vertex.tangent = tangent;
    }
}
