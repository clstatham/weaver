use std::{path::PathBuf, sync::Arc};

use nom::Finish;
use weaver_asset::{AssetCommands, prelude::*};
use weaver_core::{mesh::Mesh, prelude::Vec3};
use weaver_ecs::prelude::Commands;
use weaver_pbr::prelude::WHITE_TEXTURE;
use weaver_util::prelude::*;

use crate::{
    bsp::{
        generator::{BspFaceType, BspPlane, GenBsp, GenBspMeshNode},
        parser::{VisData, bsp_file},
    },
    shader::{
        lexer::{LexedShader, LexedShaderGlobalParam, Map, ShaderStageParam},
        loader::{
            LexedShaderCache, LoadedShader, LoadedShaderCache, TextureCache,
            TryEverythingTextureLoader, strip_extension,
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
        min: Vec3,
        max: Vec3,
    },
}

#[derive(Asset)]
pub struct Bsp {
    pub shaders: FxHashMap<PathBuf, String>,
    pub nodes: Vec<Option<BspNode>>,
    pub vis_data: VisData,
}

impl Bsp {
    pub fn with_capacity(capacity: usize, vis_data: VisData) -> Self {
        Self {
            shaders: FxHashMap::default(),
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

#[derive(Default)]
pub struct BspLoader {
    lexed_shader_cache: Lock<LexedShaderCache>,
    loaded_shader_cache: Lock<LoadedShaderCache>,
    texture_cache: Lock<TextureCache>,
}

impl BspLoader {
    async fn load_shader_from_lexed(
        &self,
        shader: LexedShader,
        fs: Arc<Filesystem>,
        commands: &Commands,
    ) -> LoadedShader {
        let mut textures = FxHashMap::default();

        let mut texture_cache = self.texture_cache.write();

        for param in &shader.global_params {
            if let LexedShaderGlobalParam::EditorImage(path) = param {
                let stripped = strip_extension(path);
                let map = Map::Path(stripped.to_string());
                if textures.contains_key(&map) {
                    continue;
                }
                if let Some(handle) = texture_cache.get(stripped) {
                    textures.insert(map, handle);
                    continue;
                }
                let handle = commands
                    .load_asset::<_, TryEverythingTextureLoader, _>((path.into(), fs.clone()))
                    .await;
                texture_cache.insert(stripped.to_string(), handle);
                textures.insert(map, handle);
            }
        }

        for stage in &shader.stages {
            for directive in &stage.params {
                if let ShaderStageParam::Map(map) = directive {
                    match map {
                        Map::Path(path) => {
                            let stripped = strip_extension(path);
                            let map = Map::Path(stripped.to_string());
                            if textures.contains_key(&map) {
                                continue;
                            }
                            if let Some(handle) = texture_cache.get(stripped) {
                                textures.insert(map, handle);
                                continue;
                            }
                            let handle = commands
                                .load_asset::<_, TryEverythingTextureLoader, _>((
                                    path.into(),
                                    fs.clone(),
                                ))
                                .await;
                            texture_cache.insert(stripped.to_string(), handle);
                            textures.insert(map, handle);
                        }
                        Map::WhiteImage => {
                            textures.insert(Map::WhiteImage, WHITE_TEXTURE);
                        }
                        Map::Lightmap => {
                            textures.insert(Map::Lightmap, WHITE_TEXTURE);
                        }
                    }
                }
            }
        }

        LoadedShader { shader, textures }
    }
}

impl Loader<Bsp, PathAndFilesystem> for BspLoader {
    // TODO: clean this up
    async fn load(&self, source: PathAndFilesystem, commands: &Commands) -> Result<Bsp> {
        let bytes = source.read()?;
        let (_, bsp_file) = bsp_file(bytes.as_slice())
            .finish()
            .map_err(|e| anyhow!("Failed to parse bsp file: {:?}", e.code))?;

        let gen_bsp = GenBsp::build(bsp_file);
        let meshes_and_textures = gen_bsp.generate_meshes();

        let mut bsp = Bsp::with_capacity(meshes_and_textures.nodes.len(), gen_bsp.file.vis_data);

        self.lexed_shader_cache
            .write()
            .load_all("scripts", &source.fs)?;

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
                        let mesh = commands.load_asset_direct(mesh).await;
                        let texture_name = texture.to_str().unwrap();
                        let texture_name = strip_extension(texture_name);

                        let mut loaded_shader_cache = self.loaded_shader_cache.write();
                        let lexed_shader_cache = self.lexed_shader_cache.read();

                        if let Some(shader) = loaded_shader_cache.get(texture_name) {
                            shader_meshes.push(LoadedBspShaderMesh { mesh, shader, typ });
                        } else if let Some(shader) = lexed_shader_cache.get(texture_name) {
                            let shader = self
                                .load_shader_from_lexed(shader.clone(), source.fs.clone(), commands)
                                .await;

                            let shader = commands.load_asset_direct(shader).await;

                            loaded_shader_cache.insert(texture_name.to_string(), shader);
                            shader_meshes.push(LoadedBspShaderMesh { mesh, shader, typ });
                        } else {
                            let texture_cache = self.texture_cache.read();
                            if let Some(texture) = texture_cache.get(texture_name) {
                                let shader =
                                    LoadedShader::make_simple_textured(texture, texture_name);
                                let shader = commands.load_asset_direct(shader).await;
                                shader_meshes.push(LoadedBspShaderMesh { mesh, shader, typ });
                            } else {
                                let texture = commands
                                    .load_asset::<_, TryEverythingTextureLoader, _>((
                                        texture.to_str().unwrap().into(),
                                        source.fs.clone(),
                                    ))
                                    .await;
                                let shader =
                                    LoadedShader::make_simple_textured(texture, texture_name);
                                let shader = commands.load_asset_direct(shader).await;
                                shader_meshes.push(LoadedBspShaderMesh { mesh, shader, typ });
                            }
                        }
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
                    mins,
                    maxs,
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
                            min: mins,
                            max: maxs,
                        },
                    );
                }
            }
        }

        Ok(bsp)
    }
}
