use super::{
    color::Color,
    mesh::{Mesh, Vertex},
};

pub fn load_obj(path: &str) -> Result<Mesh, String> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let (models, _materials) = tobj::load_obj(
        path,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ignore_lines: true,
            ignore_points: true,
        },
    )
    .map_err(|e| e.to_string())?;

    let model = &models[0];
    let mesh = &model.mesh;

    for i in 0..mesh.positions.len() / 3 {
        let x = mesh.positions[i * 3];
        let y = mesh.positions[i * 3 + 1];
        let z = mesh.positions[i * 3 + 2];

        positions.push(glam::Vec3::new(x, y, z));
    }

    for i in 0..mesh.normals.len() / 3 {
        let x = mesh.normals[i * 3];
        let y = mesh.normals[i * 3 + 1];
        let z = mesh.normals[i * 3 + 2];

        normals.push(glam::Vec3::new(x, y, z));
    }

    for (position, normal) in positions.iter().zip(normals.iter()) {
        vertices.push(Vertex {
            position: *position,
            color: Color::new(1.0, 1.0, 1.0),
            normal: Some(*normal),
        });
    }

    for i in 0..mesh.indices.len() / 3 {
        let i0 = mesh.indices[i * 3] as usize;
        let i1 = mesh.indices[i * 3 + 1] as usize;
        let i2 = mesh.indices[i * 3 + 2] as usize;

        indices.push(i0 as u32);
        indices.push(i1 as u32);
        indices.push(i2 as u32);
    }

    Ok(Mesh::new(vertices, indices))
}
