use std::{collections::HashMap, path::PathBuf};

use weaver_asset::{
    loading::{LoadAsset, LoadCtx},
    prelude::Asset,
    Assets, Handle,
};
use weaver_core::{mesh::Mesh, texture::Texture};
use weaver_ecs::{component::ResMut, prelude::Resource};
use weaver_pbr::material::Material;
use weaver_util::{
    prelude::{anyhow, Result},
    warn_once,
};

use crate::bsp::{
    generator::{BspFaceType, BspPlane, GenBsp, GenBspMeshNode},
    parser::{bsp_file, VisData},
};

#[derive(Debug, Clone)]
pub enum BspNode {
    Leaf {
        meshes_and_materials: Vec<(Handle<Mesh>, Handle<Material>, BspFaceType)>,
        parent: usize,
        cluster: usize,
    },
    Node {
        plane: BspPlane,
        back: usize,
        front: usize,
        parent: Option<usize>,
    },
}

#[derive(Asset)]
pub struct Bsp {
    pub shaders: HashMap<PathBuf, String>,
    pub nodes: Vec<Option<BspNode>>,
    pub vis_data: VisData,
}

impl Bsp {
    pub fn with_capacity(capacity: usize, vis_data: VisData) -> Self {
        Self {
            shaders: HashMap::new(),
            nodes: vec![None; capacity],
            vis_data,
        }
    }

    pub const fn root(&self) -> usize {
        0
    }

    pub fn insert(&mut self, index: usize, node: BspNode) {
        self.nodes[index] = Some(node);
    }

    pub fn node_iter(&self) -> impl Iterator<Item = (usize, &BspNode)> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| n.as_ref().map(|n| (i, n)))
    }

    pub fn leaf_iter(&self) -> impl Iterator<Item = (usize, &BspNode)> {
        self.node_iter()
            .filter(|(_, n)| matches!(n, BspNode::Leaf { .. }))
    }

    pub fn walk<F>(&self, index: usize, visitor: &mut F)
    where
        F: FnMut(&BspNode),
    {
        let mut stack = vec![index];
        while let Some(index) = stack.pop() {
            let Some(node) = &self.nodes[index] else {
                continue;
            };
            visitor(node);
            match node {
                BspNode::Leaf { .. } => {}
                BspNode::Node { back, front, .. } => {
                    stack.push(*front);
                    stack.push(*back);
                }
            }
        }
    }
}

#[derive(Default, Resource)]
pub struct BspLoader;

impl LoadAsset<Bsp> for BspLoader {
    type Param = (
        ResMut<'static, Assets<Mesh>>,
        ResMut<'static, Assets<Material>>,
        ResMut<'static, Assets<Texture>>,
    );

    // TODO: clean this up
    fn load(
        &self,
        (mut mesh_assets, mut material_assets, mut texture_assets): (
            ResMut<Assets<Mesh>>,
            ResMut<Assets<Material>>,
            ResMut<Assets<Texture>>,
        ),
        ctx: &mut LoadCtx,
    ) -> Result<Bsp> {
        let bytes = ctx.read_original()?;
        let (_, bsp_file) =
            bsp_file(bytes.as_slice()).map_err(|e| anyhow!("Failed to parse bsp file: {:?}", e))?;
        let gen = GenBsp::build(bsp_file);
        let meshes_and_textures = gen.generate_meshes();

        let mut bsp = Bsp::with_capacity(meshes_and_textures.nodes.len(), gen.file.vis_data);
        let mut material_handles = HashMap::new();

        let error_texture = Texture::from_rgba8(&[255, 0, 255, 255], 1, 1);
        let error_texture_handle = texture_assets.insert(error_texture);
        let error_material = Material::from(error_texture_handle);
        let error_material = material_assets.insert(error_material);

        for (node_index, node) in meshes_and_textures.nodes.into_iter().enumerate() {
            let Some(node) = node else {
                continue;
            };

            match node {
                GenBspMeshNode::Leaf {
                    cluster,
                    area: _,
                    mins: _,
                    maxs: _,
                    meshes_and_textures,
                    parent,
                } => {
                    let mut meshes_and_materials = Vec::with_capacity(meshes_and_textures.len());
                    for (mesh, texture, typ) in meshes_and_textures {
                        let mesh = mesh_assets.insert(mesh);
                        let texture_name = texture.name.to_string_lossy().into_owned();
                        if let Some(material) = material_handles.get(&texture_name) {
                            meshes_and_materials.push((mesh, *material, typ));
                        } else {
                            let extensions_to_try = [
                                (".tga", image::ImageFormat::Tga),
                                (".jpg", image::ImageFormat::Jpeg),
                                (".png", image::ImageFormat::Png),
                                (".dds", image::ImageFormat::Dds),
                            ];
                            let mut found_format = None;
                            let mut found_extension = None;
                            'try_load_image: for (extension, format) in extensions_to_try.iter() {
                                let texture_name_with_extension =
                                    format!("{}{}", texture_name, extension);
                                if ctx.filesystem().exists(&texture_name_with_extension) {
                                    found_format = Some(*format);
                                    found_extension = Some(extension);
                                    break 'try_load_image;
                                }
                            }

                            if let (Some(found_extension), Some(format)) =
                                (found_extension, found_format)
                            {
                                let texture_name = format!("{}{}", texture_name, found_extension);
                                let texture_bytes =
                                    ctx.filesystem().read_sub_path(&texture_name)?;
                                let texture =
                                    image::load_from_memory_with_format(&texture_bytes, format)?
                                        .to_rgba8();
                                let texture = Texture::from_rgba8(
                                    &texture,
                                    texture.width(),
                                    texture.height(),
                                );
                                let texture_handle = texture_assets.insert(texture);

                                let mut material = Material::from(texture_handle);
                                material.metallic = 0.0;
                                material.roughness = 0.5;
                                let material_handle = material_assets.insert(material);
                                material_handles.insert(texture_name.to_owned(), material_handle);
                                meshes_and_materials.push((mesh, material_handle, typ));
                            } else {
                                // just try to load it anyway
                                if let Ok(texture_bytes) =
                                    ctx.filesystem().read_sub_path(&texture_name)
                                {
                                    let texture =
                                        image::load_from_memory(&texture_bytes)?.to_rgba8();
                                    let texture = Texture::from_rgba8(
                                        &texture,
                                        texture.width(),
                                        texture.height(),
                                    );
                                    let texture_handle = texture_assets.insert(texture);

                                    let mut material = Material::from(texture_handle);
                                    material.metallic = 0.0;
                                    material.roughness = 0.5;
                                    let material_handle = material_assets.insert(material);
                                    material_handles
                                        .insert(texture_name.to_owned(), material_handle);
                                    meshes_and_materials.push((mesh, material_handle, typ));
                                } else {
                                    warn_once!(
                                        "Some textures could not be loaded, using solid pink error texture instead"
                                    );
                                    log::debug!("Failed to load texture: {:?}", texture_name);
                                    meshes_and_materials.push((mesh, error_material, typ));
                                }
                            }
                        }
                    }
                    bsp.insert(
                        node_index,
                        BspNode::Leaf {
                            meshes_and_materials,
                            parent,
                            cluster: cluster as usize,
                        },
                    );
                }
                GenBspMeshNode::Node {
                    plane,
                    mins: _,
                    maxs: _,
                    back,
                    front,
                    parent,
                } => {
                    bsp.insert(
                        node_index,
                        BspNode::Node {
                            plane,
                            back,
                            front,
                            parent,
                        },
                    );
                }
            }
        }

        Ok(bsp)
    }
}
