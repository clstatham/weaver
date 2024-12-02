use std::path::PathBuf;

use glam::{Vec2, Vec3, Vec4};
use weaver_asset::{
    prelude::{Asset, Loader},
    LoadSource, PathAndFilesystem,
};
use weaver_ecs::prelude::Commands;
use weaver_util::prelude::*;

use crate::prelude::{Aabb, Transform};

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub tangent: Vec3,
    pub tex_coords: Vec2,
}

#[derive(Clone, Asset, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub aabb: Aabb,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        for vertex in &vertices {
            min = min.min(vertex.position);
            max = max.max(vertex.position);
        }

        let aabb = Aabb::new(min, max);

        Self {
            vertices,
            indices,
            aabb,
        }
    }

    pub fn transformed(&self, transform: Transform) -> Self {
        let mut vertices = self.vertices.clone();
        let matrix = transform.matrix();
        for vertex in &mut vertices {
            vertex.position = matrix.transform_point3(vertex.position);
            vertex.normal = matrix.transform_vector3(vertex.normal);
            vertex.tangent = matrix.transform_vector3(vertex.tangent);
        }

        Self {
            vertices,
            indices: self.indices.clone(),
            aabb: self.aabb.transformed(transform),
        }
    }

    pub fn regenerate_aabb(&mut self) {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        for vertex in &self.vertices {
            min = min.min(vertex.position);
            max = max.max(vertex.position);
        }

        self.aabb = Aabb::new(min, max);
    }

    pub fn recalculate_normals(&mut self) {
        calculate_normals(&mut self.vertices, &self.indices);
    }

    pub fn recalculate_tangents(&mut self) {
        calculate_tangents(&mut self.vertices, &self.indices);
    }
}

#[derive(Default)]
pub struct ObjMeshLoader<S: LoadSource>(std::marker::PhantomData<S>);

impl Loader<Mesh, PathAndFilesystem> for ObjMeshLoader<PathBuf> {
    async fn load(&self, source: PathAndFilesystem, _commands: &Commands) -> Result<Mesh> {
        let bytes = source.read()?;
        let meshes = load_obj(&bytes)?;
        if meshes.len() != 1 {
            bail!("expected exactly one mesh in OBJ file: {:?}", source.path);
        }
        Ok(meshes.into_iter().next().unwrap())
    }
}

impl Loader<Mesh, Vec<u8>> for ObjMeshLoader<Vec<u8>> {
    async fn load(&self, source: Vec<u8>, _commands: &Commands) -> Result<Mesh> {
        load_obj(&source)?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("expected exactly one mesh in OBJ file"))
    }
}

impl Loader<Vec<Mesh>, PathAndFilesystem> for ObjMeshLoader<PathBuf> {
    async fn load(&self, source: PathAndFilesystem, _commands: &Commands) -> Result<Vec<Mesh>> {
        let bytes = source.read()?;
        load_obj(&bytes)
    }
}

impl Loader<Vec<Mesh>, Vec<u8>> for ObjMeshLoader<Vec<u8>> {
    async fn load(&self, source: Vec<u8>, _commands: &Commands) -> Result<Vec<Mesh>> {
        load_obj(&source)
    }
}

pub fn load_obj(bytes: &[u8]) -> Result<Vec<Mesh>> {
    let (models, _) = tobj::load_obj_buf(
        &mut std::io::Cursor::new(bytes),
        &tobj::LoadOptions {
            single_index: true,
            triangulate: true,
            ignore_lines: true,
            ignore_points: true,
        },
        |_| Err(tobj::LoadError::MaterialParseError),
    )?;

    if models.is_empty() {
        bail!("expected at least one model in OBJ file");
    }

    let mut meshes = Vec::with_capacity(models.len());

    for model in &models {
        let mesh = &model.mesh;

        let mut vertices = Vec::with_capacity(mesh.positions.len() / 3);
        let mut indices = Vec::with_capacity(mesh.indices.len());
        let has_normals = !mesh.normals.is_empty();

        for i in 0..mesh.positions.len() / 3 {
            let position = [
                mesh.positions[i * 3],
                mesh.positions[i * 3 + 1],
                mesh.positions[i * 3 + 2],
            ];
            let normal = if has_normals {
                [
                    mesh.normals[i * 3],
                    mesh.normals[i * 3 + 1],
                    mesh.normals[i * 3 + 2],
                ]
            } else {
                [0.0, 0.0, 0.0]
            };
            let uv = [mesh.texcoords[i * 2], 1.0 - mesh.texcoords[i * 2 + 1]];

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

        if !has_normals {
            calculate_normals(&mut vertices, &indices);
        }

        calculate_tangents(&mut vertices, &indices);

        meshes.push(Mesh::new(vertices, indices));
    }

    Ok(meshes)
}

#[derive(Default)]
pub struct GltfMeshLoader<S: LoadSource>(std::marker::PhantomData<S>);

impl Loader<Mesh, PathAndFilesystem> for GltfMeshLoader<PathBuf> {
    async fn load(&self, source: PathAndFilesystem, _commands: &Commands) -> Result<Mesh> {
        let bytes = source.read()?;
        let meshes = load_gltf(&bytes)?;
        if meshes.len() != 1 {
            bail!("expected exactly one mesh in GLTF file: {:?}", source.path);
        }
        Ok(meshes.into_iter().next().unwrap())
    }
}

impl Loader<Mesh, Vec<u8>> for GltfMeshLoader<Vec<u8>> {
    async fn load(&self, source: Vec<u8>, _commands: &Commands) -> Result<Mesh> {
        load_gltf(&source)?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("expected exactly one mesh in GLTF file"))
    }
}

impl Loader<Vec<Mesh>, PathAndFilesystem> for GltfMeshLoader<PathBuf> {
    async fn load(&self, source: PathAndFilesystem, _commands: &Commands) -> Result<Vec<Mesh>> {
        let bytes = source.read()?;
        load_gltf(&bytes)
    }
}

impl Loader<Vec<Mesh>, Vec<u8>> for GltfMeshLoader<Vec<u8>> {
    async fn load(&self, source: Vec<u8>, _commands: &Commands) -> Result<Vec<Mesh>> {
        load_gltf(&source)
    }
}

pub fn load_gltf(bytes: &[u8]) -> Result<Vec<Mesh>> {
    let (gltf, buffers, _) = gltf::import_slice(bytes)?;

    if gltf.meshes().count() == 0 {
        bail!("expected at least one mesh in GLTF file");
    }

    let mut meshes = Vec::new();

    for mesh in gltf.meshes() {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let mut positions = reader.read_positions().ok_or_else(|| {
                anyhow!("mesh primitive does not have positions: {:?}", primitive)
            })?;

            let mut normals = reader
                .read_normals()
                .ok_or_else(|| anyhow!("mesh primitive does not have normals: {:?}", primitive))?;

            let mut tex_coords = reader
                .read_tex_coords(0)
                .ok_or_else(|| anyhow!("mesh primitive does not have tex coords: {:?}", primitive))?
                .into_f32();

            let mut tangents = reader
                .read_tangents()
                .ok_or_else(|| anyhow!("mesh primitive does not have tangents: {:?}", primitive))?;

            let indices_iter = reader
                .read_indices()
                .ok_or_else(|| anyhow!("mesh primitive does not have indices: {:?}", primitive))?
                .into_u32();

            let vertices_iter = positions
                .by_ref()
                .zip(normals.by_ref())
                .zip(tex_coords.by_ref())
                .zip(tangents.by_ref())
                .map(|(((position, normal), tex_coord), tangent)| Vertex {
                    position: Vec3::from(position),
                    normal: Vec3::from(normal),
                    tex_coords: Vec2::from(tex_coord),
                    tangent: Vec4::from(tangent).truncate(),
                });

            let vertex_offset = vertices.len() as u32;

            vertices.extend(vertices_iter);
            indices.extend(indices_iter.map(|index| index + vertex_offset));
        }

        meshes.push(Mesh::new(vertices, indices));
    }

    Ok(meshes)
}

pub fn calculate_normals(vertices: &mut [Vertex], indices: &[u32]) {
    for vertex in vertices.iter_mut() {
        vertex.normal = Vec3::ZERO;
    }

    for c in indices.chunks_exact(3) {
        let i0 = c[0] as usize;
        let i1 = c[1] as usize;
        let i2 = c[2] as usize;

        let v0 = vertices[i0].position;
        let v1 = vertices[i1].position;
        let v2 = vertices[i2].position;

        let normal = (v1 - v0).cross(v2 - v0).normalize();

        vertices[i0].normal += normal;
        vertices[i1].normal += normal;
        vertices[i2].normal += normal;
    }

    for vertex in vertices.iter_mut() {
        vertex.normal = vertex.normal.normalize();
    }
}

pub fn calculate_tangents(vertices: &mut [Vertex], indices: &[u32]) {
    for vertex in vertices.iter_mut() {
        vertex.tangent = Vec3::ZERO;
    }

    let mut num_triangles = vec![0; vertices.len()];
    for c in indices.chunks_exact(3) {
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
    }
}
