use std::collections::HashMap;

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

use crate::{
    generator::{BspPlane, GenBsp, GenBspMeshNode},
    parser::{bsp_file, VisData},
};

pub type BspIndex = usize;

#[derive(Debug, Clone)]
pub enum BspNode {
    Leaf {
        meshes_and_materials: Vec<(Handle<Mesh>, Handle<Material>)>,
        parent: BspIndex,
        cluster: usize,
    },
    Node {
        plane: BspPlane,
        back: BspIndex,
        front: BspIndex,
        parent: Option<BspIndex>,
    },
}

#[derive(Asset)]
pub struct Bsp {
    pub nodes: Vec<Option<BspNode>>,
    pub vis_data: VisData,
}

impl Bsp {
    pub fn with_capacity(capacity: usize, vis_data: VisData) -> Self {
        Self {
            nodes: vec![None; capacity],
            vis_data,
        }
    }

    pub const fn root(&self) -> BspIndex {
        0
    }

    pub fn insert(&mut self, index: BspIndex, node: BspNode) {
        self.nodes[index] = Some(node);
    }

    pub fn node_iter(&self) -> impl Iterator<Item = (BspIndex, &BspNode)> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| n.as_ref().map(|n| (i, n)))
    }

    pub fn leaf_iter(&self) -> impl Iterator<Item = (BspIndex, &BspNode)> {
        self.node_iter()
            .filter(|(_, n)| matches!(n, BspNode::Leaf { .. }))
    }

    pub fn walk<F>(&self, index: BspIndex, visitor: &mut F)
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
                    for (mesh, texture) in meshes_and_textures {
                        let mesh = mesh_assets.insert(mesh);
                        let texture_name = texture.name.to_string_lossy().into_owned();
                        if let Some(material) = material_handles.get(&texture_name) {
                            meshes_and_materials.push((mesh, *material));
                        } else {
                            let extensions_to_try = [
                                (".tga", image::ImageFormat::Tga),
                                (".jpg", image::ImageFormat::Jpeg),
                                (".png", image::ImageFormat::Png),
                            ];
                            let mut found = false;
                            'try_load_image: for (extension, format) in extensions_to_try.iter() {
                                let texture_name_with_extension =
                                    format!("{}{}", texture_name, extension);
                                match ctx.root().read_sub_path(&texture_name_with_extension) {
                                    Ok(texture_bytes) => {
                                        let texture = image::load_from_memory_with_format(
                                            &texture_bytes,
                                            *format,
                                        )?
                                        .to_rgba8();
                                        let texture = Texture::from_rgba8(
                                            &texture,
                                            texture.width(),
                                            texture.height(),
                                        );
                                        let texture_handle = texture_assets.insert(texture);

                                        let mut material = Material::from(texture_handle);
                                        material.metallic = 0.0;
                                        material.roughness = 1.0;
                                        let material_handle = material_assets.insert(material);
                                        material_handles
                                            .insert(texture_name.to_owned(), material_handle);
                                        meshes_and_materials.push((mesh, material_handle));
                                        found = true;
                                        break;
                                    }
                                    Err(_) => continue 'try_load_image,
                                }
                            }

                            if !found {
                                warn_once!(
                                    "Some textures could not be loaded, using solid pink error texture instead"
                                );
                                meshes_and_materials.push((mesh, error_material));
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
