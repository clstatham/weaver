use std::ffi::{CStr, CString};

use nom::{
    bytes::complete::{tag, take},
    multi::count,
    IResult, Parser,
};
use weaver_asset::prelude::Asset;

pub fn ubyte(input: &[u8]) -> IResult<&[u8], u8> {
    nom::number::complete::le_u8(input)
}

pub fn int(input: &[u8]) -> IResult<&[u8], i32> {
    nom::number::complete::le_i32(input)
}

pub fn float(input: &[u8]) -> IResult<&[u8], f32> {
    nom::number::complete::le_f32(input)
}

pub fn string(input: &[u8]) -> IResult<&[u8], &CStr> {
    let string = CStr::from_bytes_until_nul(input).unwrap();
    Ok((&input[string.to_bytes().len()..], string))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct DirEntry {
    pub offset: i32,
    pub length: i32,
}

impl DirEntry {
    pub const fn size() -> usize {
        4 // offset
        + 4 // length
    }
}

pub fn dir_entry(input: &[u8]) -> IResult<&[u8], DirEntry> {
    let (input, offset) = int(input)?;
    let (input, length) = int(input)?;
    Ok((input, DirEntry { offset, length }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct BspHeader {
    pub magic: [u8; 4],
    pub version: i32,
    pub entities: DirEntry,
    pub textures: DirEntry,
    pub planes: DirEntry,
    pub nodes: DirEntry,
    pub leafs: DirEntry,
    pub leaf_faces: DirEntry,
    pub leaf_brushes: DirEntry,
    pub models: DirEntry,
    pub brushes: DirEntry,
    pub brush_sides: DirEntry,
    pub verts: DirEntry,
    pub mesh_verts: DirEntry,
    pub effects: DirEntry,
    pub faces: DirEntry,
    pub lightmaps: DirEntry,
    pub light_vols: DirEntry,
    pub vis_data: DirEntry,
}

impl BspHeader {
    pub const fn size() -> usize {
        4 // magic
        + 4 // version
        + DirEntry::size() // entities
        + DirEntry::size() // textures
        + DirEntry::size() // planes
        + DirEntry::size() // nodes
        + DirEntry::size() // leafs
        + DirEntry::size() // leaf_faces
        + DirEntry::size() // leaf_brushes
        + DirEntry::size() // models
        + DirEntry::size() // brushes
        + DirEntry::size() // brush_sides
        + DirEntry::size() // verts
        + DirEntry::size() // mesh_verts
        + DirEntry::size() // effects
        + DirEntry::size() // faces
        + DirEntry::size() // lightmaps
        + DirEntry::size() // light_vols
        + DirEntry::size() // vis_data
    }
}

pub fn bsp_header(input: &[u8]) -> IResult<&[u8], BspHeader> {
    let (input, magic) = tag(b"IBSP")(input)?;
    let (input, version) = int(input)?;
    let (input, entities) = dir_entry(input)?;
    let (input, textures) = dir_entry(input)?;
    let (input, planes) = dir_entry(input)?;
    let (input, nodes) = dir_entry(input)?;
    let (input, leafs) = dir_entry(input)?;
    let (input, leaf_faces) = dir_entry(input)?;
    let (input, leaf_brushes) = dir_entry(input)?;
    let (input, models) = dir_entry(input)?;
    let (input, brushes) = dir_entry(input)?;
    let (input, brush_sides) = dir_entry(input)?;
    let (input, verts) = dir_entry(input)?;
    let (input, mesh_verts) = dir_entry(input)?;
    let (input, effects) = dir_entry(input)?;
    let (input, faces) = dir_entry(input)?;
    let (input, lightmaps) = dir_entry(input)?;
    let (input, light_vols) = dir_entry(input)?;
    let (input, vis_data) = dir_entry(input)?;
    Ok((
        input,
        BspHeader {
            magic: [magic[0], magic[1], magic[2], magic[3]],
            version,
            entities,
            textures,
            planes,
            nodes,
            leafs,
            leaf_faces,
            leaf_brushes,
            models,
            brushes,
            brush_sides,
            verts,
            mesh_verts,
            effects,
            faces,
            lightmaps,
            light_vols,
            vis_data,
        },
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Entities {
    pub data: CString,
}

pub fn entities(input: &[u8]) -> IResult<&[u8], Entities> {
    let (input, data) = string(input)?;
    Ok((
        input,
        Entities {
            data: data.to_owned(),
        },
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Texture {
    pub name: CString,
    pub flags: i32,
    pub contents: i32,
}

impl Texture {
    pub const fn size() -> usize {
        64 // name
        + 4 // flags
        + 4 // contents
    }
}

pub fn texture(input: &[u8]) -> IResult<&[u8], Texture> {
    let (input, name) = take(64usize)(input)?;
    let (_, name) = string(name)?;
    let (input, flags) = int(input)?;
    let (input, contents) = int(input)?;
    Ok((
        input,
        Texture {
            name: name.to_owned(),
            flags,
            contents,
        },
    ))
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Plane {
    pub normal: [f32; 3],
    pub dist: f32,
}

impl Plane {
    pub const fn size() -> usize {
        4 * 3 // normal
        + 4 // dist
    }
}

pub fn plane(input: &[u8]) -> IResult<&[u8], Plane> {
    let (input, normal) = count(float, 3)(input)?;
    let (input, dist) = float(input)?;
    Ok((
        input,
        Plane {
            normal: [normal[0], normal[1], normal[2]],
            dist,
        },
    ))
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Node {
    pub plane: i32,
    pub children: [i32; 2],
    pub mins: [i32; 3],
    pub maxs: [i32; 3],
}

impl Node {
    pub const fn size() -> usize {
        4 // plane
        + 4 * 2 // children
        + 4 * 3 // mins
        + 4 * 3 // maxs
    }
}

pub fn node(input: &[u8]) -> IResult<&[u8], Node> {
    let (input, plane) = int(input)?;
    let (input, children) = count(int, 2)(input)?;
    let (input, mins) = count(int, 3)(input)?;
    let (input, maxs) = count(int, 3)(input)?;
    Ok((
        input,
        Node {
            plane,
            children: [children[0], children[1]],
            mins: [mins[0], mins[1], mins[2]],
            maxs: [maxs[0], maxs[1], maxs[2]],
        },
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Leaf {
    pub cluster: i32,
    pub area: i32,
    pub mins: [i32; 3],
    pub maxs: [i32; 3],
    pub leaf_face: i32,
    pub num_leaf_faces: i32,
    pub leaf_brush: i32,
    pub num_leaf_brushes: i32,
}

impl Leaf {
    pub const fn size() -> usize {
        4 // cluster
        + 4 // area
        + 4 * 3 // mins
        + 4 * 3 // maxs
        + 4 // leaf_face
        + 4 // num_leaf_faces
        + 4 // leaf_brush
        + 4 // num_leaf_brushes
    }
}

pub fn leaf(input: &[u8]) -> IResult<&[u8], Leaf> {
    let (input, cluster) = int(input)?;
    let (input, area) = int(input)?;
    let (input, mins) = count(int, 3)(input)?;
    let (input, maxs) = count(int, 3)(input)?;
    let (input, leaf_face) = int(input)?;
    let (input, num_leaf_faces) = int(input)?;
    let (input, leaf_brush) = int(input)?;
    let (input, num_leaf_brushes) = int(input)?;
    Ok((
        input,
        Leaf {
            cluster,
            area,
            mins: [mins[0], mins[1], mins[2]],
            maxs: [maxs[0], maxs[1], maxs[2]],
            leaf_face,
            num_leaf_faces,
            leaf_brush,
            num_leaf_brushes,
        },
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct LeafFace {
    pub face: i32,
}

impl LeafFace {
    pub const fn size() -> usize {
        4 // face
    }
}

pub fn leaf_face(input: &[u8]) -> IResult<&[u8], LeafFace> {
    let (input, face) = int(input)?;
    Ok((input, LeafFace { face }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct LeafBrush {
    pub brush: i32,
}

impl LeafBrush {
    pub const fn size() -> usize {
        4 // brush
    }
}

pub fn leaf_brush(input: &[u8]) -> IResult<&[u8], LeafBrush> {
    let (input, brush) = int(input)?;
    Ok((input, LeafBrush { brush }))
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Model {
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub face: i32,
    pub num_faces: i32,
    pub brush: i32,
    pub num_brushes: i32,
}

impl Model {
    pub const fn size() -> usize {
        4 * 3 // mins
        + 4 * 3 // maxs
        + 4 // face
        + 4 // num_faces
        + 4 // brush
        + 4 // num_brushes
    }
}

pub fn model(input: &[u8]) -> IResult<&[u8], Model> {
    let (input, mins) = count(float, 3)(input)?;
    let (input, maxs) = count(float, 3)(input)?;
    let (input, face) = int(input)?;
    let (input, num_faces) = int(input)?;
    let (input, brush) = int(input)?;
    let (input, num_brushes) = int(input)?;
    Ok((
        input,
        Model {
            mins: [mins[0], mins[1], mins[2]],
            maxs: [maxs[0], maxs[1], maxs[2]],
            face,
            num_faces,
            brush,
            num_brushes,
        },
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Brush {
    pub brush_side: i32,
    pub num_brush_sides: i32,
    pub texture: i32,
}

impl Brush {
    pub const fn size() -> usize {
        4 // brush_side
        + 4 // num_brush_sides
        + 4 // texture
    }
}

pub fn brush(input: &[u8]) -> IResult<&[u8], Brush> {
    let (input, brush_side) = int(input)?;
    let (input, num_brush_sides) = int(input)?;
    let (input, texture) = int(input)?;
    Ok((
        input,
        Brush {
            brush_side,
            num_brush_sides,
            texture,
        },
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct BrushSide {
    pub plane: i32,
    pub texture: i32,
}

impl BrushSide {
    pub const fn size() -> usize {
        4 // plane
        + 4 // texture
    }
}

pub fn brush_side(input: &[u8]) -> IResult<&[u8], BrushSide> {
    let (input, plane) = int(input)?;
    let (input, texture) = int(input)?;
    Ok((input, BrushSide { plane, texture }))
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Vert {
    pub position: [f32; 3],
    pub surface_tex_coord: [f32; 2],
    pub lightmap_tex_coord: [f32; 2],
    pub normal: [f32; 3],
    pub color: [u8; 4],
}

impl Vert {
    pub const fn size() -> usize {
        4 * 3 // position
        + 4 * 2 // surface_tex_coord
        + 4 * 2 // lightmap_tex_coord
        + 4 * 3 // normal
        + 4 // color
    }
}

pub fn vert(input: &[u8]) -> IResult<&[u8], Vert> {
    let (input, position) = count(float, 3)(input)?;
    let (input, surface_tex_coord) = count(float, 2)(input)?;
    let (input, lightmap_tex_coord) = count(float, 2)(input)?;
    let (input, normal) = count(float, 3)(input)?;
    let (input, color) = count(ubyte, 4)(input)?;
    Ok((
        input,
        Vert {
            position: [position[0], position[1], position[2]],
            surface_tex_coord: [surface_tex_coord[0], surface_tex_coord[1]],
            lightmap_tex_coord: [lightmap_tex_coord[0], lightmap_tex_coord[1]],
            normal: [normal[0], normal[1], normal[2]],
            color: [color[0], color[1], color[2], color[3]],
        },
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct MeshVert {
    pub offset: i32,
}

impl MeshVert {
    pub const fn size() -> usize {
        4 // offset
    }
}

pub fn mesh_vert(input: &[u8]) -> IResult<&[u8], MeshVert> {
    let (input, offset) = int(input)?;
    Ok((input, MeshVert { offset }))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Effect {
    pub name: CString,
    pub brush: i32,
    pub unknown: i32,
}

impl Effect {
    pub const fn size() -> usize {
        64 // name
        + 4 // brush
        + 4 // unknown
    }
}

pub fn effect(input: &[u8]) -> IResult<&[u8], Effect> {
    let (input, name) = take(64usize)(input)?;
    let (_, name) = string(name)?;
    let (input, brush) = int(input)?;
    let (input, unknown) = int(input)?;
    Ok((
        input,
        Effect {
            name: name.to_owned(),
            brush,
            unknown,
        },
    ))
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Face {
    pub texture: i32,
    pub effect: i32,
    pub typ: i32,
    pub first_vertex: i32,
    pub num_verts: i32,
    pub first_mesh_vert: i32,
    pub num_mesh_verts: i32,
    pub lightmap: i32,
    pub lightmap_start: [i32; 2],
    pub lightmap_size: [i32; 2],
    pub lightmap_origin: [f32; 3],
    pub lightmap_s: [f32; 3],
    pub lightmap_t: [f32; 3],
    pub normal: [f32; 3],
    pub size: [i32; 2],
}

impl Face {
    pub const fn size() -> usize {
        4 // texture
        + 4 // effect
        + 4 // typ
        + 4 // first_vertex
        + 4 // num_verts
        + 4 // first_mesh_vert
        + 4 // num_mesh_verts
        + 4 // lightmap
        + 4 * 2 // lightmap_start
        + 4 * 2 // lightmap_size
        + 4 * 3 // lightmap_origin
        + 4 * 3 // lightmap_s
        + 4 * 3 // lightmap_t
        + 4 * 3 // normal
        + 4 * 2 // size
    }
}

pub fn face(input: &[u8]) -> IResult<&[u8], Face> {
    let (input, texture) = int(input)?;
    let (input, effect) = int(input)?;
    let (input, type_) = int(input)?;
    let (input, first_vertex) = int(input)?;
    let (input, num_verts) = int(input)?;
    let (input, first_mesh_vert) = int(input)?;
    let (input, num_mesh_verts) = int(input)?;
    let (input, lightmap) = int(input)?;
    let (input, lightmap_start) = count(int, 2)(input)?;
    let (input, lightmap_size) = count(int, 2)(input)?;
    let (input, lightmap_origin) = count(float, 3)(input)?;
    let (input, lightmap_s) = count(float, 3)(input)?;
    let (input, lightmap_t) = count(float, 3)(input)?;
    let (input, normal) = count(float, 3)(input)?;
    let (input, size) = count(int, 2)(input)?;
    Ok((
        input,
        Face {
            texture,
            effect,
            typ: type_,
            first_vertex,
            num_verts,
            first_mesh_vert,
            num_mesh_verts,
            lightmap,
            lightmap_start: [lightmap_start[0], lightmap_start[1]],
            lightmap_size: [lightmap_size[0], lightmap_size[1]],
            lightmap_origin: [lightmap_origin[0], lightmap_origin[1], lightmap_origin[2]],
            lightmap_s: [lightmap_s[0], lightmap_s[1], lightmap_s[2]],
            lightmap_t: [lightmap_t[0], lightmap_t[1], lightmap_t[2]],
            normal: [normal[0], normal[1], normal[2]],
            size: [size[0], size[1]],
        },
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Lightmap {
    pub map: Vec<Vec<[u8; 3]>>,
}

impl Lightmap {
    pub const fn size() -> usize {
        128 * 128 * 3
    }
}

pub fn lightmap(input: &[u8]) -> IResult<&[u8], Lightmap> {
    #[rustfmt::skip]
    let (input, map) = count(
        count(
            count(ubyte, 3)
                .map(|rgb| [rgb[0], rgb[1], rgb[2]]),
            128),
        128,
    )(input)?;
    Ok((input, Lightmap { map }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct LightVol {
    pub ambient: [u8; 3],
    pub directional: [u8; 3],
    pub direction: [u8; 2],
}

impl LightVol {
    pub const fn size() -> usize {
        3 // ambient
        + 3 // directional
        + 2 // direction
    }
}

pub fn light_vol(input: &[u8]) -> IResult<&[u8], LightVol> {
    let (input, ambient) = count(ubyte, 3)(input)?;
    let (input, directional) = count(ubyte, 3)(input)?;
    let (input, direction) = count(ubyte, 2)(input)?;
    Ok((
        input,
        LightVol {
            ambient: [ambient[0], ambient[1], ambient[2]],
            directional: [directional[0], directional[1], directional[2]],
            direction: [direction[0], direction[1]],
        },
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct VisData {
    pub num_vecs: i32,
    pub size_vecs: i32,
    pub vecs: Vec<u8>,
}

impl VisData {
    /// Checks if two clusters are visible to each other.
    pub fn check_visible(&self, cluster1: i32, cluster2: i32) -> bool {
        if cluster1 < 0 || cluster2 < 0 {
            return false;
        }

        let vis_row = cluster1 * self.size_vecs + cluster2 / 8;
        let vis_byte = cluster2 % 8;

        let vis_data = self.vecs[vis_row as usize];
        vis_data & (1 << vis_byte) != 0
    }
}

pub fn vis_data(input: &[u8]) -> IResult<&[u8], VisData> {
    let (input, num_vecs) = int(input)?;
    let (input, size_vecs) = int(input)?;
    let (input, vecs) = take(num_vecs as usize * size_vecs as usize)(input)?;
    Ok((
        input,
        VisData {
            num_vecs,
            size_vecs,
            vecs: vecs.to_vec(),
        },
    ))
}

#[derive(Debug, Clone, PartialEq, Asset)]
#[repr(C)]
pub struct BspFile {
    pub header: BspHeader,
    pub entities: Entities,
    pub textures: Vec<Texture>,
    pub planes: Vec<Plane>,
    pub nodes: Vec<Node>,
    pub leafs: Vec<Leaf>,
    pub leaf_faces: Vec<LeafFace>,
    pub leaf_brushes: Vec<LeafBrush>,
    pub models: Vec<Model>,
    pub brushes: Vec<Brush>,
    pub brush_sides: Vec<BrushSide>,
    pub verts: Vec<Vert>,
    pub mesh_verts: Vec<MeshVert>,
    pub effects: Vec<Effect>,
    pub faces: Vec<Face>,
    pub lightmaps: Vec<Lightmap>,
    pub light_vols: Vec<LightVol>,
    pub vis_data: VisData,
}

pub fn bsp_file(input: &[u8]) -> IResult<&[u8], BspFile> {
    let (_, header) = bsp_header(input)?;

    let BspHeader {
        magic: _,
        version,
        entities,
        textures,
        planes,
        nodes,
        leafs,
        leaf_faces,
        leaf_brushes,
        models,
        brushes,
        brush_sides,
        verts,
        mesh_verts,
        effects,
        faces,
        lightmaps,
        light_vols,
        vis_data,
    } = header;

    if version != 46 {
        log::error!("Unsupported bsp version {}", version);
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if textures.length % Texture::size() as i32 != 0 {
        log::error!("Texture data length is not a multiple of texture size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if planes.length % Plane::size() as i32 != 0 {
        log::error!("Plane data length is not a multiple of plane size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if nodes.length % Node::size() as i32 != 0 {
        log::error!("Node data length is not a multiple of node size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if leafs.length % Leaf::size() as i32 != 0 {
        log::error!("Leaf data length is not a multiple of leaf size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if leaf_faces.length % LeafFace::size() as i32 != 0 {
        log::error!("Leaf face data length is not a multiple of leaf face size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if leaf_brushes.length % LeafBrush::size() as i32 != 0 {
        log::error!("Leaf brush data length is not a multiple of leaf brush size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if models.length % Model::size() as i32 != 0 {
        log::error!("Model data length is not a multiple of model size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if brushes.length % Brush::size() as i32 != 0 {
        log::error!("Brush data length is not a multiple of brush size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if brush_sides.length % BrushSide::size() as i32 != 0 {
        log::error!("Brush side data length is not a multiple of brush side size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if verts.length % Vert::size() as i32 != 0 {
        log::error!("Vertex data length is not a multiple of vertex size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if mesh_verts.length % MeshVert::size() as i32 != 0 {
        log::error!("Mesh vertex data length is not a multiple of mesh vertex size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if effects.length % Effect::size() as i32 != 0 {
        log::error!("Effect data length is not a multiple of effect size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if faces.length % Face::size() as i32 != 0 {
        log::error!("Face data length is not a multiple of face size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if lightmaps.length % Lightmap::size() as i32 != 0 {
        log::error!("Lightmap data length is not a multiple of lightmap size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    if light_vols.length % LightVol::size() as i32 != 0 {
        log::error!("Light volume data length is not a multiple of light volume size");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let total_length = BspHeader::size()
        + entities.length as usize
        + textures.length as usize
        + planes.length as usize
        + nodes.length as usize
        + leafs.length as usize
        + leaf_faces.length as usize
        + leaf_brushes.length as usize
        + models.length as usize
        + brushes.length as usize
        + brush_sides.length as usize
        + verts.length as usize
        + mesh_verts.length as usize
        + effects.length as usize
        + faces.length as usize
        + lightmaps.length as usize
        + light_vols.length as usize
        + vis_data.length as usize;

    if total_length != input.len() {
        log::warn!(
            "Total length of all data ({} bytes) does not match input length ({} bytes)",
            total_length,
            input.len()
        );
    }

    let entity_bytes =
        &input[entities.offset as usize..(entities.offset + entities.length) as usize];
    let textures_bytes =
        &input[textures.offset as usize..(textures.offset + textures.length) as usize];
    let planes_bytes = &input[planes.offset as usize..(planes.offset + planes.length) as usize];
    let nodes_bytes = &input[nodes.offset as usize..(nodes.offset + nodes.length) as usize];
    let leafs_bytes = &input[leafs.offset as usize..(leafs.offset + leafs.length) as usize];
    let leaf_faces_bytes =
        &input[leaf_faces.offset as usize..(leaf_faces.offset + leaf_faces.length) as usize];
    let leaf_brushes_bytes =
        &input[leaf_brushes.offset as usize..(leaf_brushes.offset + leaf_brushes.length) as usize];
    let models_bytes = &input[models.offset as usize..(models.offset + models.length) as usize];
    let brushes_bytes = &input[brushes.offset as usize..(brushes.offset + brushes.length) as usize];
    let brush_sides_bytes =
        &input[brush_sides.offset as usize..(brush_sides.offset + brush_sides.length) as usize];
    let verts_bytes = &input[verts.offset as usize..(verts.offset + verts.length) as usize];
    let mesh_verts_bytes =
        &input[mesh_verts.offset as usize..(mesh_verts.offset + mesh_verts.length) as usize];
    let effects_bytes = &input[effects.offset as usize..(effects.offset + effects.length) as usize];
    let faces_bytes = &input[faces.offset as usize..(faces.offset + faces.length) as usize];
    let lightmaps_bytes =
        &input[lightmaps.offset as usize..(lightmaps.offset + lightmaps.length) as usize];
    let light_vols_bytes =
        &input[light_vols.offset as usize..(light_vols.offset + light_vols.length) as usize];
    let vis_data_bytes =
        &input[vis_data.offset as usize..(vis_data.offset + vis_data.length) as usize];

    let (_rest, entities) = crate::bsp::parser::entities(entity_bytes)?;
    // if !rest.is_empty() && !entity_bytes.is_empty() {
    //     log::error!("Failed to parse all entities");
    //     return Err(nom::Err::Error(nom::error::Error::new(
    //         input,
    //         nom::error::ErrorKind::Verify,
    //     )));
    // }

    let (rest, textures_vec) = count(
        crate::bsp::parser::texture,
        textures.length as usize / Texture::size(),
    )(textures_bytes)?;
    if !rest.is_empty() && !textures_bytes.is_empty() {
        log::error!("Failed to parse all textures");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (rest, planes_vec) = count(
        crate::bsp::parser::plane,
        planes.length as usize / Plane::size(),
    )(planes_bytes)?;
    if !rest.is_empty() && !planes_bytes.is_empty() {
        log::error!("Failed to parse all planes");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (rest, nodes_vec) = count(
        crate::bsp::parser::node,
        nodes.length as usize / Node::size(),
    )(nodes_bytes)?;
    if !rest.is_empty() && !nodes_bytes.is_empty() {
        log::error!("Failed to parse all nodes");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (rest, leafs_vec) = count(
        crate::bsp::parser::leaf,
        leafs.length as usize / Leaf::size(),
    )(leafs_bytes)?;
    if !rest.is_empty() && !leafs_bytes.is_empty() {
        log::error!("Failed to parse all leafs");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (rest, leaf_faces_vec) = count(
        crate::bsp::parser::leaf_face,
        leaf_faces.length as usize / LeafFace::size(),
    )(leaf_faces_bytes)?;
    if !rest.is_empty() && !leaf_faces_bytes.is_empty() {
        log::error!("Failed to parse all leaf faces");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (rest, leaf_brushes_vec) = count(
        crate::bsp::parser::leaf_brush,
        leaf_brushes.length as usize / LeafBrush::size(),
    )(leaf_brushes_bytes)?;
    if !rest.is_empty() && !leaf_brushes_bytes.is_empty() {
        log::error!("Failed to parse all leaf brushes");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (rest, models_vec) = count(
        crate::bsp::parser::model,
        models.length as usize / Model::size(),
    )(models_bytes)?;
    if !rest.is_empty() && !models_bytes.is_empty() {
        log::error!("Failed to parse all models");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (rest, brushes_vec) = count(
        crate::bsp::parser::brush,
        brushes.length as usize / Brush::size(),
    )(brushes_bytes)?;
    if !rest.is_empty() && !brushes_bytes.is_empty() {
        log::error!("Failed to parse all brushes");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (rest, brush_sides_vec) = count(
        crate::bsp::parser::brush_side,
        brush_sides.length as usize / BrushSide::size(),
    )(brush_sides_bytes)?;
    if !rest.is_empty() && !brush_sides_bytes.is_empty() {
        log::error!("Failed to parse all brush sides");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (rest, verts_vec) = count(
        crate::bsp::parser::vert,
        verts.length as usize / Vert::size(),
    )(verts_bytes)?;
    if !rest.is_empty() && !verts_bytes.is_empty() {
        log::error!("Failed to parse all verts");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (rest, mesh_verts_vec) = count(
        crate::bsp::parser::mesh_vert,
        mesh_verts.length as usize / MeshVert::size(),
    )(mesh_verts_bytes)?;
    if !rest.is_empty() && !mesh_verts_bytes.is_empty() {
        log::error!("Failed to parse all mesh verts");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (rest, effects_vec) = count(
        crate::bsp::parser::effect,
        effects.length as usize / Effect::size(),
    )(effects_bytes)?;
    if !rest.is_empty() && !effects_bytes.is_empty() {
        log::error!("Failed to parse all effects");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (_, faces_vec) = count(
        crate::bsp::parser::face,
        faces.length as usize / Face::size(),
    )(faces_bytes)?;
    if !rest.is_empty() && !faces_bytes.is_empty() {
        log::error!("Failed to parse all faces");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (_, lightmaps_vec) = count(
        crate::bsp::parser::lightmap,
        lightmaps.length as usize / Lightmap::size(),
    )(lightmaps_bytes)?;
    if !rest.is_empty() && !lightmaps_bytes.is_empty() {
        log::error!("Failed to parse all lightmaps");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let (_, light_vols_vec) = count(
        crate::bsp::parser::light_vol,
        light_vols.length as usize / LightVol::size(),
    )(light_vols_bytes)?;
    if !rest.is_empty() && !light_vols_bytes.is_empty() {
        log::error!("Failed to parse all light volumes");
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let vis_data = if vis_data.length == 0 {
        VisData {
            num_vecs: 0,
            size_vecs: 0,
            vecs: vec![],
        }
    } else {
        let (_, vis_data) = crate::bsp::parser::vis_data(vis_data_bytes)?;
        if !rest.is_empty() && !vis_data_bytes.is_empty() {
            log::error!("Failed to parse vis data");
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Verify,
            )));
        }
        vis_data
    };

    Ok((
        input,
        BspFile {
            header,
            entities,
            textures: textures_vec,
            planes: planes_vec,
            nodes: nodes_vec,
            leafs: leafs_vec,
            leaf_faces: leaf_faces_vec,
            leaf_brushes: leaf_brushes_vec,
            models: models_vec,
            brushes: brushes_vec,
            brush_sides: brush_sides_vec,
            verts: verts_vec,
            mesh_verts: mesh_verts_vec,
            effects: effects_vec,
            faces: faces_vec,
            lightmaps: lightmaps_vec,
            light_vols: light_vols_vec,
            vis_data,
        },
    ))
}
