use std::ffi::CString;
use std::ops::{Add, Mul};

use weaver_core::mesh::{Mesh, Vertex};
use weaver_core::prelude::*;
use weaver_ecs::prelude::Component;

use crate::bsp::parser::{Brush, BspFile, Face, Node, Vert};

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

#[derive(Debug, Clone, Copy)]
pub struct BspPlane {
    pub normal: Vec3,
    pub distance: f32,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GenBspVertex {
    pub position: Vec3,
    pub tex_coords: Vec2,
    pub lightmap_coords: Vec2,
    pub normal: Vec3,
    pub color: [u8; 4],
}

impl GenBspVertex {
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
        let term0 = p0 * (1.0 - t) * (1.0 - t);
        let term1 = p1 * 2.0 * (1.0 - t) * t;
        let term2 = p2 * t * t;
        term0 + term1 + term2
    }
}

impl Add<GenBspVertex> for GenBspVertex {
    type Output = Self;

    fn add(self, rhs: GenBspVertex) -> Self::Output {
        Self::Output {
            position: self.position + rhs.position,
            tex_coords: self.tex_coords + rhs.tex_coords,
            lightmap_coords: self.lightmap_coords + rhs.lightmap_coords,
            normal: self.normal + rhs.normal,
            color: [
                (self.color[0] as f32 + rhs.color[0] as f32) as u8,
                (self.color[1] as f32 + rhs.color[1] as f32) as u8,
                (self.color[2] as f32 + rhs.color[2] as f32) as u8,
                (self.color[3] as f32 + rhs.color[3] as f32) as u8,
            ],
        }
    }
}

impl Mul<f32> for GenBspVertex {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::Output {
            position: self.position * rhs,
            tex_coords: self.tex_coords * rhs,
            lightmap_coords: self.lightmap_coords * rhs,
            normal: self.normal * rhs,
            color: [
                (self.color[0] as f32 * rhs) as u8,
                (self.color[1] as f32 * rhs) as u8,
                (self.color[2] as f32 * rhs) as u8,
                (self.color[3] as f32 * rhs) as u8,
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum BspFaceType {
    Polygon,
    Patch,
    Mesh,
    Billboard,
}

#[derive(Debug)]
pub struct GenBspEffect {
    pub name: CString,
    pub brush: i32,
}

#[derive(Debug, Clone)]
pub struct GenBspTexture {
    pub name: CString,
    pub flags: i32,
    pub contents: i32,
}

#[derive(Debug)]
pub struct GenBspFace {
    pub face_index: u32,
    pub texture: GenBspTexture,
    pub effect: Option<GenBspEffect>,
    pub typ: BspFaceType,
    pub verts: Vec<GenBspVertex>,
    pub mesh_verts: Vec<u32>,
    pub lightmap: Option<Vec<Vec<[u8; 3]>>>,
    pub lightmap_origin: Vec3,
    pub lightmap_s: Vec3,
    pub lightmap_t: Vec3,
    pub normal: Vec3,
    pub size: [i32; 2],
}

impl GenBspFace {
    pub fn build(file: &BspFile, face: &Face, face_index: u32) -> Self {
        let texture = &file.textures[face.texture as usize];
        let effect = if face.effect < 0 || face.effect as usize >= file.effects.len() {
            None
        } else {
            let effect = &file.effects[face.effect as usize];
            Some(GenBspEffect {
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

        let verts: Vec<GenBspVertex> = (face.first_vertex..face.first_vertex + face.num_verts)
            .map(|i| &file.verts[i as usize])
            .map(GenBspVertex::build)
            .collect();

        let mesh_verts: Vec<u32> = (face.first_mesh_vert
            ..face.first_mesh_vert + face.num_mesh_verts)
            .map(|i| file.mesh_verts[i as usize].offset as u32)
            .collect();

        let lightmap = if face.lightmap < 0 || face.lightmap as usize >= file.lightmaps.len() {
            None
        } else {
            Some(file.lightmaps[face.lightmap as usize].map.clone())
        };

        Self {
            face_index,
            texture: GenBspTexture {
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

#[derive(Debug)]
pub struct BspPatch {
    pub verts: Vec<GenBspVertex>,
    pub control_points: Vec<GenBspVertex>,
    pub rows: i32,
    pub row_indices: Vec<Vec<u32>>,
}

impl BspPatch {
    // algorithm credit (very loose translation from the source C++ code)
    // https://github.com/codesuki/bsp-renderer/blob/94a739e06278632c5442954442664f7b26fd4643/src/bsp.cpp#L152
    // https://github.com/codesuki/bsp-renderer/blob/94a739e06278632c5442954442664f7b26fd4643/src/bezier.cpp#L32
    #[allow(clippy::needless_range_loop)]
    pub fn build(control_points: &[GenBspVertex], subdivisions: usize) -> Self {
        assert_eq!(control_points.len(), 9);
        let mut verts = vec![GenBspVertex::default(); (subdivisions + 1) * (subdivisions + 1)];

        let mut temp = [GenBspVertex::default(); 3];

        for i in 0..=subdivisions {
            let l = i as f32 / subdivisions as f32;
            for j in 0..3 {
                let k = j * 3;
                temp[j] = GenBspVertex::quadratic_bezier(
                    control_points[k],
                    control_points[k + 1],
                    control_points[k + 2],
                    l,
                );
            }

            for j in 0..=subdivisions {
                let a = j as f32 / subdivisions as f32;

                let p0 = GenBspVertex::quadratic_bezier(temp[0], temp[1], temp[2], a);
                verts[i * (subdivisions + 1) + j] = p0;
            }
        }

        let mut indices = vec![0; subdivisions * (subdivisions + 1) * 2];

        for row in 0..subdivisions {
            for col in 0..=subdivisions {
                let h = (row * (subdivisions + 1) + col) * 2;
                indices[h] = ((row + 1) * (subdivisions + 1) + col) as u32;
                indices[h + 1] = (row * (subdivisions + 1) + col) as u32;
            }
        }

        let tris_per_row = 2 * (subdivisions + 1);
        let mut row_indices = vec![vec![0; tris_per_row]; subdivisions];
        // for row in 0..subdivisions {
        //     row_indices[row] = indices[row * tris_per_row..(row + 1) * tris_per_row].to_vec();
        // }

        // convert the triangle strip to triangle list
        for row in 0..subdivisions {
            let mut triangle_indices = Vec::new();
            for col in 0..subdivisions {
                let i0 = row * (subdivisions + 1) + col;
                let i1 = i0 + 1;
                let i2 = (row + 1) * (subdivisions + 1) + col;
                let i3 = i2 + 1;

                triangle_indices.push(i0 as u32);
                triangle_indices.push(i2 as u32);
                triangle_indices.push(i1 as u32);

                triangle_indices.push(i1 as u32);
                triangle_indices.push(i2 as u32);
                triangle_indices.push(i3 as u32);
            }
            row_indices[row] = triangle_indices;
        }

        Self {
            verts,
            control_points: control_points.to_vec(),
            rows: subdivisions as i32,
            row_indices,
        }
    }
}

#[derive(Debug)]
pub struct BspBrushSide {
    pub plane: BspPlane,
    pub texture: GenBspTexture,
}

#[derive(Debug)]
pub struct BspBrush {
    pub sides: Vec<BspBrushSide>,
    pub texture: GenBspTexture,
}

impl BspBrush {
    pub fn build(file: &BspFile, brush: &Brush) -> Self {
        let sides = (brush.brush_side..brush.brush_side + brush.num_brush_sides)
            .map(|i| file.brush_sides[i as usize].plane)
            .map(|i| &file.planes[i as usize])
            .map(|plane| BspPlane {
                normal: q3_to_weaver().transform_vector3(plane.normal.into()),
                distance: plane.dist,
            })
            .zip(
                (brush.brush_side..brush.brush_side + brush.num_brush_sides)
                    .map(|i| file.brush_sides[i as usize].texture)
                    .map(|i| &file.textures[i as usize]),
            )
            .map(|(plane, texture)| BspBrushSide {
                plane,
                texture: GenBspTexture {
                    name: texture.name.to_owned(),
                    flags: texture.flags,
                    contents: texture.contents,
                },
            })
            .collect();

        let texture = &file.textures[brush.texture as usize];
        Self {
            sides,
            texture: GenBspTexture {
                name: texture.name.to_owned(),
                flags: texture.flags,
                contents: texture.contents,
            },
        }
    }
}

#[derive(Debug)]
pub enum GenBspNode {
    Node {
        index: usize,
        plane: BspPlane,
        mins: Vec3,
        maxs: Vec3,
        children: [Box<GenBspNode>; 2],
    },
    Leaf {
        index: usize,
        cluster: i32,
        area: i32,
        mins: Vec3,
        maxs: Vec3,
        leaf_faces: Vec<GenBspFace>,
        leaf_brushes: Vec<BspBrush>,
    },
}

impl GenBspNode {
    pub fn build(file: &BspFile) -> Self {
        let root = file.nodes.first().unwrap();
        GenBspNode::build_recursive(file, root, 0)
    }

    fn build_recursive(file: &BspFile, node: &Node, index: usize) -> Self {
        let child1 = if node.children[0] < 0 {
            let index = -(node.children[0] + 1) as usize;
            let leaf = &file.leafs[index];
            GenBspNode::Leaf {
                index: index + file.nodes.len(),
                cluster: leaf.cluster,
                area: leaf.area,
                mins: q3_to_weaver().transform_point3(int3_to_vec3(leaf.mins)),
                maxs: q3_to_weaver().transform_point3(int3_to_vec3(leaf.maxs)),
                leaf_faces: (leaf.leaf_face..leaf.leaf_face + leaf.num_leaf_faces)
                    .map(|i| file.leaf_faces[i as usize].face)
                    .map(|i| GenBspFace::build(file, &file.faces[i as usize], i as u32))
                    .collect(),
                leaf_brushes: (leaf.leaf_brush..leaf.leaf_brush + leaf.num_leaf_brushes)
                    .map(|i| file.leaf_brushes[i as usize].brush)
                    .map(|i| &file.brushes[i as usize])
                    .map(|brush| BspBrush::build(file, brush))
                    .collect(),
            }
        } else {
            let child = &file.nodes[node.children[0] as usize];
            GenBspNode::build_recursive(file, child, node.children[0] as usize)
        };

        let child2 = if node.children[1] < 0 {
            let index = -(node.children[1] + 1) as usize;
            let leaf = &file.leafs[index];
            GenBspNode::Leaf {
                index: index + file.nodes.len(),
                cluster: leaf.cluster,
                area: leaf.area,
                mins: q3_to_weaver().transform_point3(int3_to_vec3(leaf.mins)),
                maxs: q3_to_weaver().transform_point3(int3_to_vec3(leaf.maxs)),
                leaf_faces: (leaf.leaf_face..leaf.leaf_face + leaf.num_leaf_faces)
                    .map(|i| file.leaf_faces[i as usize].face)
                    .map(|i| GenBspFace::build(file, &file.faces[i as usize], i as u32))
                    .collect(),
                leaf_brushes: (leaf.leaf_brush..leaf.leaf_brush + leaf.num_leaf_brushes)
                    .map(|i| file.leaf_brushes[i as usize].brush)
                    .map(|i| &file.brushes[i as usize])
                    .map(|brush| BspBrush::build(file, brush))
                    .collect(),
            }
        } else {
            let child = &file.nodes[node.children[1] as usize];
            GenBspNode::build_recursive(file, child, node.children[1] as usize)
        };

        let plane = &file.planes[node.plane as usize];
        GenBspNode::Node {
            index,
            plane: BspPlane {
                normal: q3_to_weaver().transform_vector3(plane.normal.into()),
                distance: plane.dist,
            },
            mins: q3_to_weaver().transform_point3(int3_to_vec3(node.mins)),
            maxs: q3_to_weaver().transform_point3(int3_to_vec3(node.maxs)),
            children: [Box::new(child1), Box::new(child2)],
        }
    }

    pub fn index(&self) -> usize {
        match self {
            GenBspNode::Node { index, .. } => *index,
            GenBspNode::Leaf { index, .. } => *index,
        }
    }
}

pub trait MeshExt {
    fn add_vertices(&mut self, vertices: &[GenBspVertex]);
    fn add_indices(&mut self, indices: &[u32]);
    fn add_polygon(&mut self, vertices: &[GenBspVertex], indices: &[u32]);
}

impl MeshExt for Mesh {
    fn add_vertices(&mut self, vertices: &[GenBspVertex]) {
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

    fn add_polygon(&mut self, vertices: &[GenBspVertex], indices: &[u32]) {
        let mut indices = indices.to_vec();
        // chop off any extra indices
        indices.truncate(indices.len() - (indices.len() % 3));

        // fix winding order based on normal
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

#[derive(Clone)]
pub enum GenBspMeshNode {
    Node {
        plane: BspPlane,
        mins: Vec3,
        maxs: Vec3,
        back: usize,
        front: usize,
        parent: Option<usize>,
    },
    Leaf {
        cluster: i32,
        area: i32,
        mins: Vec3,
        maxs: Vec3,
        meshes_and_textures: Vec<(Mesh, CString, BspFaceType)>,
        parent: usize,
    },
}

#[derive(Default)]
pub struct GenBspMeshes {
    pub nodes: Vec<Option<GenBspMeshNode>>,
}

impl GenBspMeshes {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: vec![None; capacity],
        }
    }

    pub fn insert(&mut self, index: usize, node: GenBspMeshNode) {
        self.nodes[index] = Some(node);
    }

    pub fn walk<F>(&self, index: usize, visitor: &mut F)
    where
        F: FnMut(usize, &GenBspMeshNode),
    {
        if let Some(node) = &self.nodes[index] {
            visitor(index, node);
            match node {
                GenBspMeshNode::Node { back, front, .. } => {
                    self.walk(*back, visitor);
                    self.walk(*front, visitor);
                }
                GenBspMeshNode::Leaf { .. } => {}
            }
        }
    }
}

#[derive(Debug)]
pub struct GenBsp {
    pub file: BspFile,
    pub root: GenBspNode,
}

impl GenBsp {
    pub fn build(file: BspFile) -> Self {
        Self {
            root: GenBspNode::build(&file),
            file,
        }
    }

    pub fn generate_meshes(&self) -> GenBspMeshes {
        let mut meshes = GenBspMeshes::with_capacity(self.file.leafs.len() + self.file.nodes.len());
        let mut seen_faces = Vec::new();
        Self::generate_meshes_recursive(&self.root, None, &mut meshes, &mut seen_faces);
        meshes
    }

    fn generate_meshes_recursive(
        node: &GenBspNode,
        parent: Option<usize>,
        meshes: &mut GenBspMeshes,
        seen_faces: &mut Vec<u32>,
    ) {
        match node {
            GenBspNode::Node {
                index,
                children,
                plane,
                mins,
                maxs,
            } => {
                meshes.insert(
                    *index,
                    GenBspMeshNode::Node {
                        plane: *plane,
                        mins: *mins,
                        maxs: *maxs,
                        back: children[0].index(),
                        front: children[1].index(),
                        parent,
                    },
                );
                Self::generate_meshes_recursive(&children[0], Some(*index), meshes, seen_faces);
                Self::generate_meshes_recursive(&children[1], Some(*index), meshes, seen_faces);
            }
            GenBspNode::Leaf {
                leaf_faces,
                index,
                cluster,
                area,
                mins,
                maxs,
                leaf_brushes: _,
            } => {
                let mut meshes_and_textures = Vec::new();
                for face in leaf_faces {
                    if seen_faces.contains(&face.face_index) {
                        continue;
                    } else {
                        seen_faces.push(face.face_index);
                    }

                    match face.typ {
                        BspFaceType::Polygon | BspFaceType::Mesh => {
                            let mut mesh = Mesh::default();
                            mesh.add_polygon(&face.verts, &face.mesh_verts);
                            mesh.regenerate_aabb();
                            mesh.recalculate_tangents();
                            if let Some(ref effect) = face.effect {
                                log::debug!("face effect: {:?}", effect);
                                meshes_and_textures.push((mesh, effect.name.clone(), face.typ));
                            } else {
                                meshes_and_textures.push((
                                    mesh,
                                    face.texture.name.clone(),
                                    face.typ,
                                ));
                            }
                        }
                        BspFaceType::Patch => {
                            let num_patches_x = (face.size[0] - 1) / 2;
                            let num_patches_y = (face.size[1] - 1) / 2;
                            let mut patches = Vec::new();
                            for y in 0..num_patches_y {
                                for x in 0..num_patches_x {
                                    let mut control_points = Vec::new();
                                    for row in 0..3 {
                                        for col in 0..3 {
                                            let index = (y * 2 * face.size[0] + x * 2)
                                                + row * face.size[0]
                                                + col;
                                            let control_point = face.verts[index as usize];
                                            control_points.push(control_point);
                                        }
                                    }

                                    let patch = BspPatch::build(&control_points, 10);
                                    patches.push(patch);
                                }
                            }

                            for patch in patches {
                                let mut mesh = Mesh::default();
                                mesh.add_vertices(&patch.verts);
                                for row in 0..patch.rows {
                                    mesh.add_indices(&patch.row_indices[row as usize]);
                                }
                                mesh.regenerate_aabb();
                                mesh.recalculate_tangents();
                                if let Some(ref effect) = face.effect {
                                    log::debug!("patch effect: {:?}", effect);
                                    meshes_and_textures.push((mesh, effect.name.clone(), face.typ));
                                } else {
                                    meshes_and_textures.push((
                                        mesh,
                                        face.texture.name.clone(),
                                        face.typ,
                                    ));
                                }
                            }
                        }
                        BspFaceType::Billboard => {
                            // todo
                        }
                    }
                }

                meshes.insert(
                    *index,
                    GenBspMeshNode::Leaf {
                        cluster: *cluster,
                        area: *area,
                        mins: *mins,
                        maxs: *maxs,
                        meshes_and_textures,
                        parent: parent.unwrap(),
                    },
                );
            }
        }
    }
}
