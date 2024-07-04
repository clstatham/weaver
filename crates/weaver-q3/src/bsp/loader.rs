use std::{collections::HashMap, path::PathBuf};

use weaver_asset::{
    loading::{LoadAsset, LoadCtx},
    prelude::Asset,
    Assets, Handle,
};
use weaver_core::{mesh::Mesh, prelude::Vec3, texture::Texture};
use weaver_ecs::prelude::Resource;
use weaver_renderer::prelude::wgpu;
use weaver_util::{
    prelude::{anyhow, Result},
    warn_once,
};

use crate::{
    bsp::{
        generator::{BspFaceType, BspPlane, GenBsp, GenBspMeshNode},
        parser::{bsp_file, VisData},
    },
    shader::{
        lexer::LexedShader,
        loader::{
            strip_extension, LoadedShader, ShaderCache, TextureCache, TryEverythingTextureLoader,
            ERROR_SHADER_HANDLE,
        },
    },
};

#[derive(Debug, Clone)]
pub struct LoadedBspShaderMesh {
    pub mesh: Handle<Mesh>,
    pub shader: Handle<LoadedShader>,
    pub typ: BspFaceType,
}

#[derive(Debug, Clone)]
pub enum BspNode {
    Leaf {
        shader_meshes: Vec<LoadedBspShaderMesh>,
        parent: usize,
        cluster: usize,
        min: Vec3,
        max: Vec3,
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
    // TODO: clean this up
    fn load(&self, ctx: &mut LoadCtx<'_, '_>) -> Result<Bsp> {
        let bytes = ctx.read_original()?;
        let (_, bsp_file) = bsp_file(bytes.as_slice())
            .map_err(|_| anyhow!("Failed to parse bsp file; check logs"))?;

        log::debug!("Loaded bsp file version {}", bsp_file.header.version);
        log::debug!(">>> Header: {:#?}", bsp_file.header);
        log::debug!(">>> {} textures", bsp_file.textures.len());
        log::debug!(">>> {} planes", bsp_file.planes.len());
        log::debug!(">>> {} nodes", bsp_file.nodes.len());
        log::debug!(">>> {} leafs", bsp_file.leafs.len());
        log::debug!(">>> {} leaf faces", bsp_file.leaf_faces.len());
        log::debug!(">>> {} leaf brushes", bsp_file.leaf_brushes.len());
        log::debug!(">>> {} models", bsp_file.models.len());
        log::debug!(">>> {} brushes", bsp_file.brushes.len());
        log::debug!(">>> {} brush sides", bsp_file.brush_sides.len());
        log::debug!(">>> {} vertices", bsp_file.verts.len());
        log::debug!(">>> {} mesh vertices", bsp_file.mesh_verts.len());
        log::debug!(">>> {} effects", bsp_file.effects.len());
        log::debug!(">>> {} faces", bsp_file.faces.len());
        log::debug!(">>> {} lightmaps", bsp_file.lightmaps.len());
        log::debug!(">>> {} light volumes", bsp_file.light_vols.len());
        log::debug!(
            ">>> {} vis data vecs of {} bytes each",
            bsp_file.vis_data.num_vecs,
            bsp_file.vis_data.size_vecs
        );

        let gen = GenBsp::build(bsp_file);
        let meshes_and_textures = gen.generate_meshes();

        let mut bsp = Bsp::with_capacity(meshes_and_textures.nodes.len(), gen.file.vis_data);

        let mut mesh_assets = ctx.get_resource_mut::<Assets<Mesh>>()?;

        for (node_index, node) in meshes_and_textures.nodes.into_iter().enumerate() {
            let Some(node) = node else {
                continue;
            };

            match node {
                GenBspMeshNode::Leaf {
                    cluster,
                    area: _,
                    mins,
                    maxs,
                    meshes_and_textures,
                    parent,
                } => {
                    let mut shader_meshes = Vec::with_capacity(meshes_and_textures.len());
                    for (mesh, texture, typ) in meshes_and_textures {
                        let mesh = mesh_assets.insert(mesh);
                        let texture_name = texture.to_str().unwrap();
                        let texture_name = strip_extension(texture_name);

                        let topology = match typ {
                            BspFaceType::Polygon | BspFaceType::Mesh | BspFaceType::Billboard => {
                                wgpu::PrimitiveTopology::TriangleList
                            }
                            BspFaceType::Patch => wgpu::PrimitiveTopology::TriangleStrip,
                        };

                        let shader_cache = ctx.get_resource::<ShaderCache>()?;
                        let lexed_shader_assets = ctx.get_resource::<Assets<LexedShader>>()?;
                        let mut loaded_shader_assets =
                            ctx.get_resource_mut::<Assets<LoadedShader>>()?;
                        if let Some(shader) = shader_cache.get(texture_name) {
                            let shader = lexed_shader_assets.get(shader).unwrap().clone();

                            let shader = LoadedShader::load_from_lexed(shader, ctx, topology);
                            let shader = loaded_shader_assets.insert(shader);
                            shader_meshes.push(LoadedBspShaderMesh { mesh, shader, typ });
                        } else {
                            let texture_cache = ctx.get_resource::<TextureCache>()?;
                            if let Some(texture) = texture_cache.get(texture_name) {
                                let shader = LoadedShader::make_simple_textured(
                                    texture,
                                    texture_name,
                                    topology,
                                );
                                let shader = loaded_shader_assets.insert(shader);
                                shader_meshes.push(LoadedBspShaderMesh { mesh, shader, typ });
                                ctx.drop_resource(texture_cache);
                            } else {
                                ctx.drop_resource(texture_cache);

                                // try to load it again
                                match ctx.load_asset::<_, TryEverythingTextureLoader>(texture_name)
                                {
                                    Ok(texture) => {
                                        log::debug!("Loaded texture: {}", texture_name);
                                        let mut texture_assets =
                                            ctx.get_resource_mut::<Assets<Texture>>()?;
                                        let handle = texture_assets.insert(texture);
                                        ctx.drop_resource_mut(texture_assets);
                                        let mut texture_cache =
                                            ctx.get_resource_mut::<TextureCache>()?;
                                        texture_cache.insert(texture_name.to_string(), handle);
                                        ctx.drop_resource_mut(texture_cache);

                                        let shader = LoadedShader::make_simple_textured(
                                            handle,
                                            texture_name,
                                            topology,
                                        );

                                        let shader = loaded_shader_assets.insert(shader);
                                        shader_meshes.push(LoadedBspShaderMesh {
                                            mesh,
                                            shader,
                                            typ,
                                        });
                                    }
                                    Err(_) => {
                                        warn_once!("Failed to load texture: {}", texture_name);
                                        shader_meshes.push(LoadedBspShaderMesh {
                                            mesh,
                                            shader: ERROR_SHADER_HANDLE,
                                            typ,
                                        });
                                    }
                                };
                            }
                        }
                        ctx.drop_resource(shader_cache);
                        ctx.drop_resource(lexed_shader_assets);
                        ctx.drop_resource_mut(loaded_shader_assets);
                    }
                    bsp.insert(
                        node_index,
                        BspNode::Leaf {
                            shader_meshes,
                            parent,
                            cluster: cluster as usize,
                            min: mins,
                            max: maxs,
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

        ctx.drop_resource_mut(mesh_assets);

        Ok(bsp)
    }
}
