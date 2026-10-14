//! CPU-side meshes and the loaders that produce them.
//!
//! A [`Mesh`] is a vertex list plus a triangle index list and a bounding box. The renderer uploads
//! it to the GPU separately. [`load_obj`] and [`load_gltf`] parse file contents into meshes, and
//! [`ObjMeshLoader`] and [`GltfMeshLoader`] wrap them as asset loaders (registered by
//! [`CoreTypesPlugin`](crate::CoreTypesPlugin)).

use std::path::PathBuf;

use glam::{Vec2, Vec3, Vec4};
use weaver_asset::{
    LoadSource, PathAndFilesystem,
    prelude::{Asset, LoadFrom},
};
use weaver_ecs::prelude::Commands;
use weaver_util::prelude::*;

use crate::prelude::Aabb;

/// A single mesh vertex, laid out to be copied straight into a GPU vertex buffer.
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    /// Position in model space.
    pub position: Vec3,
    /// Unit surface normal.
    pub normal: Vec3,
    /// Tangent direction (along increasing `u`) for normal mapping.
    pub tangent: Vec3,
    /// Texture coordinates, with `v` pointing down (image convention).
    pub tex_coords: Vec2,
}

/// A triangle mesh in CPU memory.
///
/// Triangles are described by `indices`, three entries each.
#[derive(Clone, Asset, Default)]
pub struct Mesh {
    /// Vertex data.
    pub vertices: Vec<Vertex>,
    /// Triangle list, three indices into `vertices` per triangle.
    pub indices: Vec<u32>,
    /// Bounds of `vertices` in model space. Kept up to date by `new` and `regenerate_aabb`.
    pub aabb: Aabb,
}

impl Mesh {
    /// Creates a mesh and computes its bounding box from the vertex positions.
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

    /// Recomputes [`Mesh::aabb`] after the vertex positions have been edited.
    pub fn regenerate_aabb(&mut self) {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        for vertex in &self.vertices {
            min = min.min(vertex.position);
            max = max.max(vertex.position);
        }

        self.aabb = Aabb::new(min, max);
    }

    /// Replaces the vertex normals with smooth normals computed from the triangles.
    pub fn recalculate_normals(&mut self) {
        calculate_normals(&mut self.vertices, &self.indices);
    }

    /// Replaces the vertex tangents with ones computed from positions and tex coords.
    pub fn recalculate_tangents(&mut self) {
        calculate_tangents(&mut self.vertices, &self.indices);
    }
}

/// Asset loader for Wavefront OBJ files, producing a single [`Mesh`].
///
/// The type parameter is a tag for the source this registered loader reads from; the loaders for
/// `PathAndFilesystem` and `Vec<u8>` are the ones registered. The OBJ must contain exactly one
/// model when loaded by path.
pub struct ObjMeshLoader<S: LoadSource>(std::marker::PhantomData<S>);

impl<S: LoadSource> Default for ObjMeshLoader<S> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl LoadFrom<PathAndFilesystem> for ObjMeshLoader<PathAndFilesystem> {
    type Asset = Mesh;

    fn load(&self, source: PathAndFilesystem, _commands: &Commands) -> Result<Mesh> {
        let bytes = source.read()?;
        let meshes = load_obj(&bytes)?;
        if meshes.len() != 1 {
            bail!("expected exactly one mesh in OBJ file: {:?}", source.path);
        }
        Ok(meshes.into_iter().next().unwrap())
    }
}

impl LoadFrom<Vec<u8>> for ObjMeshLoader<Vec<u8>> {
    type Asset = Mesh;

    fn load(&self, source: Vec<u8>, _commands: &Commands) -> Result<Mesh> {
        load_obj(&source)?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("expected exactly one mesh in OBJ file"))
    }
}

/// Parses OBJ file contents into one [`Mesh`] per model in the file.
///
/// Faces are triangulated and indices unified. Texture coordinates are flipped vertically to match
/// image-space `v`. Normals are computed if the file has none, and tangents are always computed.
///
/// Material libraries are not read.
///
/// # Errors
///
/// Returns an error if the OBJ can't be parsed or contains no models.
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
            // FIXME: panics (index out of bounds) when the OBJ has no texture coordinates.
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

/// Asset loader for glTF and GLB files, producing a single [`Mesh`].
///
/// The type parameter is a tag for the source this registered loader reads from; the loaders for
/// `PathBuf` (which loads from a `PathAndFilesystem`) and `Vec<u8>` are the ones registered. When
/// loading by path the file must contain exactly one mesh.
#[derive(Default)]
pub struct GltfMeshLoader<S: LoadSource>(std::marker::PhantomData<S>);

impl LoadFrom<PathAndFilesystem> for GltfMeshLoader<PathBuf> {
    type Asset = Mesh;

    fn load(&self, source: PathAndFilesystem, _commands: &Commands) -> Result<Mesh> {
        let bytes = source.read()?;
        let meshes = load_gltf(&bytes)?;
        if meshes.len() != 1 {
            bail!("expected exactly one mesh in GLTF file: {:?}", source.path);
        }
        Ok(meshes.into_iter().next().unwrap())
    }
}

impl LoadFrom<Vec<u8>> for GltfMeshLoader<Vec<u8>> {
    type Asset = Mesh;

    fn load(&self, source: Vec<u8>, _commands: &Commands) -> Result<Mesh> {
        load_gltf(&source)?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("expected exactly one mesh in GLTF file"))
    }
}

/// Parses glTF or GLB contents into one [`Mesh`] per glTF mesh, merging its primitives.
///
/// # Errors
///
/// Returns an error if the file can't be imported, contains no meshes, or a primitive lacks
/// positions, normals, tex coords (set 0), tangents or indices.
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

/// Overwrites the vertex normals with the average of the face normals of the triangles using each
/// vertex.
pub fn calculate_normals(vertices: &mut [Vertex], indices: &[u32]) {
    for vertex in vertices.iter_mut() {
        vertex.normal = Vec3::ZERO;
    }

    for c in indices.as_chunks::<3>().0 {
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

/// Overwrites `vertices`' tangents with the average of the tangents of the triangles using them.
///
/// The tangents come from the position and tex coord deltas of each triangle. They aren't
/// normalized or orthogonalized against the normal.
pub fn calculate_tangents(vertices: &mut [Vertex], indices: &[u32]) {
    for vertex in vertices.iter_mut() {
        vertex.tangent = Vec3::ZERO;
    }

    let mut num_triangles = vec![0; vertices.len()];
    for c in indices.as_chunks::<3>().0 {
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
