use std::ffi::CString;

use weaver_asset::prelude::Asset;
use weaver_core::mesh::{Mesh, Vertex};
use weaver_core::prelude::*;

use crate::parser::{Brush, BspFile, Face, Node, Vert};

#[rustfmt::skip]
pub fn q3_to_weaver() -> Mat4 {
    Mat4::from_cols_array(&[
        1.0, 0.0, 0.0, 0.0,
        0.0, 0.0, -1.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ])
}

pub const fn int3_to_vec3(int3: [i32; 3]) -> Vec3 {
    Vec3::new(int3[0] as f32, int3[1] as f32, int3[2] as f32)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BspPlane {
    pub normal: Vec3,
    pub distance: f32,
}

#[derive(Debug, Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct BspVertex {
    pub position: Vec3,
    pub tex_coords: Vec2,
    pub lightmap_coords: Vec2,
    pub normal: Vec3,
    pub color: [u8; 4],
}

impl BspVertex {
    pub fn build(vertex: &Vert) -> Self {
        Self {
            position: vertex.position.into(),
            tex_coords: vertex.surface_tex_coord.into(),
            lightmap_coords: vertex.lightmap_tex_coord.into(),
            normal: vertex.normal.into(),
            color: vertex.color,
        }
    }

    pub fn lerp(a: Self, b: Self, t: f32) -> Self {
        let color = [
            (a.color[0] as f32 * (1.0 - t) + b.color[0] as f32 * t) as u8,
            (a.color[1] as f32 * (1.0 - t) + b.color[1] as f32 * t) as u8,
            (a.color[2] as f32 * (1.0 - t) + b.color[2] as f32 * t) as u8,
            (a.color[3] as f32 * (1.0 - t) + b.color[3] as f32 * t) as u8,
        ];
        Self {
            position: a.position.lerp(b.position, t),
            tex_coords: a.tex_coords.lerp(b.tex_coords, t),
            lightmap_coords: a.lightmap_coords.lerp(b.lightmap_coords, t),
            normal: a.normal.lerp(b.normal, t),
            color,
        }
    }

    pub fn quadratic_bezier(p0: Self, p1: Self, p2: Self, t: f32) -> Self {
        let p01 = Self::lerp(p0, p1, t);
        let p12 = Self::lerp(p1, p2, t);
        Self::lerp(p01, p12, t)
    }
}

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BspFaceType {
    Polygon,
    Patch,
    Mesh,
    Billboard,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BspEffect {
    pub name: CString,
    pub brush: i32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BspTexture {
    pub name: CString,
    pub flags: i32,
    pub contents: i32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BspFace {
    pub texture: BspTexture,
    pub effect: Option<BspEffect>,
    pub typ: BspFaceType,
    pub verts: Vec<BspVertex>,
    pub mesh_verts: Vec<u32>,
    pub lightmap: Option<Vec<Vec<[u8; 3]>>>,
    pub lightmap_origin: Vec3,
    pub lightmap_s: Vec3,
    pub lightmap_t: Vec3,
    pub normal: Vec3,
    pub size: [i32; 2],
}

impl BspFace {
    pub fn build(file: &BspFile, face: &Face) -> Self {
        let texture = &file.textures[face.texture as usize];
        let effect = if face.effect < 0 || face.effect as usize >= file.effects.len() {
            None
        } else {
            let effect = &file.effects[face.effect as usize];
            Some(BspEffect {
                name: effect.name.to_owned(),
                brush: effect.brush,
            })
        };

        let typ = if face.typ == 1 {
            BspFaceType::Polygon
        } else if face.typ == 2 {
            BspFaceType::Patch
        } else if face.typ == 3 {
            BspFaceType::Mesh
        } else {
            BspFaceType::Billboard
        };

        let verts: Vec<BspVertex> = (face.first_vertex..face.first_vertex + face.num_verts)
            .map(|i| &file.verts[i as usize])
            .map(BspVertex::build)
            .collect();

        let mesh_verts: Vec<u32> = (face.first_mesh_vert
            ..face.first_mesh_vert + face.num_mesh_verts)
            .map(|i| file.mesh_verts[i as usize].offset as u32)
            .collect();

        let lightmap = if face.lightmap < 0 {
            None
        } else {
            Some(file.lightmaps[face.lightmap as usize].map.clone())
        };

        Self {
            texture: BspTexture {
                name: texture.name.to_owned(),
                flags: texture.flags,
                contents: texture.contents,
            },
            effect,
            typ,
            verts,
            mesh_verts,
            lightmap,
            lightmap_origin: face.lightmap_origin.into(),
            lightmap_s: face.lightmap_s.into(),
            lightmap_t: face.lightmap_t.into(),
            normal: face.normal.into(),
            size: face.size,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BspPatch {
    pub verts: Vec<BspVertex>,
    pub control_points: Vec<BspVertex>,
    pub indices: Vec<u32>,
    pub rows: i32,
    pub tris_per_row: Vec<usize>,
}

impl BspPatch {
    // algorithm credit (very loose translation from the source C++ code)
    // https://github.com/codesuki/bsp-renderer/blob/94a739e06278632c5442954442664f7b26fd4643/src/bsp.cpp#L152
    // https://github.com/codesuki/bsp-renderer/blob/94a739e06278632c5442954442664f7b26fd4643/src/bezier.cpp#L32
    #[allow(clippy::needless_range_loop)]
    pub fn build(face: &BspFace, subdivisions: usize) -> Self {
        assert_eq!(face.typ, BspFaceType::Patch);

        let width = face.size[0] as usize;
        let height = face.size[1] as usize;
        let width_count = (width - 1) / 2;
        let height_count = (height - 1) / 2;

        let mut control_points = vec![BspVertex::default(); 9];

        for y in 0..height_count {
            for x in 0..width_count {
                for row in 0..3 {
                    for col in 0..3 {
                        let index = (y * 2 * width + x * 2) + row * width + col;
                        let control_point = face.verts[index];
                        control_points[row * 3 + col] = control_point;
                    }
                }
            }
        }

        let mut verts = vec![BspVertex::default(); (subdivisions + 1) * (subdivisions + 1)];
        let mut indices = vec![0; subdivisions * (subdivisions + 1) * 2];

        let mut temp = [BspVertex::default(); 3];

        for i in 0..subdivisions + 1 {
            let l = i as f32 / subdivisions as f32;
            for j in 0..3 {
                let k = j * 3;
                temp[j] = BspVertex::quadratic_bezier(
                    control_points[k],
                    control_points[k + 1],
                    control_points[k + 2],
                    l,
                );
            }

            for j in 0..subdivisions + 1 {
                let a = j as f32 / subdivisions as f32;

                let p0 = BspVertex::quadratic_bezier(temp[0], temp[1], temp[2], a);
                verts[i * (subdivisions + 1) + j] = p0;
            }
        }

        for row in 0..subdivisions {
            for col in 0..subdivisions + 1 {
                let h = (row * (subdivisions + 1) + col) * 2;
                let g = h + 1;
                indices[h] = (row * (subdivisions + 1) + col) as u32;
                indices[g] = ((row + 1) * (subdivisions + 1) + col) as u32;
            }
        }

        let mut tris_per_row = vec![0; subdivisions];
        for _ in 0..subdivisions {
            tris_per_row[0] = 2 * (subdivisions + 1);
        }

        Self {
            verts,
            control_points,
            indices,
            rows: subdivisions as i32,
            tris_per_row,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BspBrushSide {
    pub plane: BspPlane,
    pub texture: BspTexture,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BspBrush {
    pub sides: Vec<BspBrushSide>,
    pub texture: BspTexture,
}

impl BspBrush {
    pub fn build(file: &BspFile, brush: &Brush) -> Self {
        let sides = (brush.brush_side..brush.brush_side + brush.num_brush_sides)
            .map(|i| file.brush_sides[i as usize].plane)
            .map(|i| &file.planes[i as usize])
            .map(|plane| BspPlane {
                normal: plane.normal.into(),
                distance: plane.dist,
            })
            .zip(
                (brush.brush_side..brush.brush_side + brush.num_brush_sides)
                    .map(|i| file.brush_sides[i as usize].texture)
                    .map(|i| &file.textures[i as usize]),
            )
            .map(|(plane, texture)| BspBrushSide {
                plane,
                texture: BspTexture {
                    name: texture.name.to_owned(),
                    flags: texture.flags,
                    contents: texture.contents,
                },
            })
            .collect();

        let texture = &file.textures[brush.texture as usize];
        Self {
            sides,
            texture: BspTexture {
                name: texture.name.to_owned(),
                flags: texture.flags,
                contents: texture.contents,
            },
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum BspNode {
    Node {
        plane: BspPlane,
        mins: Vec3,
        maxs: Vec3,
        children: [Box<BspNode>; 2],
    },
    Leaf {
        cluster: i32,
        area: i32,
        mins: Vec3,
        maxs: Vec3,
        leaf_faces: Vec<BspFace>,
        leaf_brushes: Vec<BspBrush>,
    },
}

impl BspNode {
    pub fn build(file: &BspFile) -> Self {
        let root = file.nodes.first().unwrap();
        BspNode::build_recursive(file, root)
    }

    fn build_recursive(file: &BspFile, node: &Node) -> Self {
        let child1 = if node.children[0] < 0 {
            let leaf = &file.leafs[(-node.children[0] - 1) as usize];
            BspNode::Leaf {
                cluster: leaf.cluster,
                area: leaf.area,
                mins: int3_to_vec3(leaf.mins),
                maxs: int3_to_vec3(leaf.maxs),
                leaf_faces: (leaf.leaf_face..leaf.leaf_face + leaf.num_leaf_faces)
                    .map(|i| file.leaf_faces[i as usize].face)
                    .map(|i| &file.faces[i as usize])
                    .map(|face| BspFace::build(file, face))
                    .collect(),
                leaf_brushes: (leaf.leaf_brush..leaf.leaf_brush + leaf.num_leaf_brushes)
                    .map(|i| file.leaf_brushes[i as usize].brush)
                    .map(|i| &file.brushes[i as usize])
                    .map(|brush| BspBrush::build(file, brush))
                    .collect(),
            }
        } else {
            let child = &file.nodes[node.children[0] as usize];
            BspNode::build_recursive(file, child)
        };

        let child2 = if node.children[1] < 0 {
            let leaf = &file.leafs[(-node.children[1] - 1) as usize];
            BspNode::Leaf {
                cluster: leaf.cluster,
                area: leaf.area,
                mins: int3_to_vec3(leaf.mins),
                maxs: int3_to_vec3(leaf.maxs),
                leaf_faces: (leaf.leaf_face..leaf.leaf_face + leaf.num_leaf_faces)
                    .map(|i| file.leaf_faces[i as usize].face)
                    .map(|i| &file.faces[i as usize])
                    .map(|face| BspFace::build(file, face))
                    .collect(),
                leaf_brushes: (leaf.leaf_brush..leaf.leaf_brush + leaf.num_leaf_brushes)
                    .map(|i| file.leaf_brushes[i as usize].brush)
                    .map(|i| &file.brushes[i as usize])
                    .map(|brush| BspBrush::build(file, brush))
                    .collect(),
            }
        } else {
            let child = &file.nodes[node.children[1] as usize];
            BspNode::build_recursive(file, child)
        };

        let plane = &file.planes[node.plane as usize];
        BspNode::Node {
            plane: BspPlane {
                normal: plane.normal.into(),
                distance: plane.dist,
            },
            mins: int3_to_vec3(node.mins),
            maxs: int3_to_vec3(node.maxs),
            children: [Box::new(child1), Box::new(child2)],
        }
    }
}

pub trait MeshExt {
    fn add_vertices(&mut self, vertices: &[BspVertex]);
    fn add_indices(&mut self, indices: &[u32]);
    fn add_polygon(&mut self, vertices: &[BspVertex], indices: &[u32]);
}

impl MeshExt for Mesh {
    fn add_vertices(&mut self, vertices: &[BspVertex]) {
        for vert in vertices {
            self.vertices.push(Vertex {
                position: q3_to_weaver().transform_point3(vert.position),
                tex_coords: vert.tex_coords,
                normal: q3_to_weaver().transform_vector3(vert.normal),
                tangent: Vec3::ZERO,
            })
        }
    }

    fn add_indices(&mut self, indices: &[u32]) {
        self.indices.extend(indices);
    }

    fn add_polygon(&mut self, vertices: &[BspVertex], indices: &[u32]) {
        // fix winding order based on normal
        let mut indices = indices.to_vec();
        // chop off any extra indices
        indices.truncate(indices.len() - (indices.len() % 3));
        for idxs in indices.chunks_exact_mut(3) {
            let v0 = vertices[idxs[0] as usize];
            let v1 = vertices[idxs[1] as usize];
            let v2 = vertices[idxs[2] as usize];

            let normal = (v1.position - v0.position)
                .cross(v2.position - v0.position)
                .normalize();
            if normal.dot(v0.normal) < 0.0 {
                idxs.swap(0, 2);
            }
        }

        self.add_vertices(vertices);
        self.add_indices(&indices);
    }
}

#[derive(Debug, Asset, serde::Serialize, serde::Deserialize)]
pub struct Bsp {
    pub root: BspNode,
}

impl Bsp {
    pub fn build(file: &BspFile) -> Self {
        Self {
            root: BspNode::build(file),
        }
    }

    pub fn generate_meshes(&self) -> Vec<Mesh> {
        let mut meshes = Vec::new();
        Self::generate_meshes_recursive(&self.root, &mut meshes);
        for mesh in &mut meshes {
            mesh.regenerate_aabb();
            // mesh.recalculate_normals();
            mesh.recalculate_tangents();
        }
        meshes
    }

    fn generate_meshes_recursive(node: &BspNode, meshes: &mut Vec<Mesh>) {
        match node {
            BspNode::Node { children, .. } => {
                Self::generate_meshes_recursive(&children[0], meshes);
                Self::generate_meshes_recursive(&children[1], meshes);
            }
            BspNode::Leaf { leaf_faces, .. } => {
                for face in leaf_faces {
                    let mut mesh = Mesh::default();
                    match face.typ {
                        BspFaceType::Polygon | BspFaceType::Mesh => {
                            mesh.add_polygon(&face.verts, &face.mesh_verts);
                        }
                        BspFaceType::Patch => {
                            // let patch = BspPatch::build(face, 10);
                            // mesh.add_polygon(&patch.verts, &patch.indices);
                        }
                        BspFaceType::Billboard => {
                            // todo
                        }
                    }
                    meshes.push(mesh);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::*;

    #[test]
    fn test_bsp() {
        let bytes = include_bytes!("../../../assets/maps/oa_dm1.bsp");
        let (_rest, bsp) = bsp_file(bytes).unwrap();
        let bsp = Bsp::build(&bsp);
        let meshes = bsp.generate_meshes();
        dbg!(meshes.len());
    }
}
