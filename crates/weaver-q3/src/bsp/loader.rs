use std::path::PathBuf;

use nom::Finish;
use weaver_asset::{prelude::Asset, AssetLoadQueues, Filesystem, Handle, LoadSource, Loader, Url};
use weaver_core::{mesh::Mesh, prelude::Vec3, texture::Texture};
use weaver_ecs::prelude::Resource;
use weaver_pbr::prelude::WHITE_TEXTURE;
use weaver_util::{anyhow, FxHashMap, Lock, Result};

use crate::{
    bsp::{
        generator::{BspFaceType, BspPlane, GenBsp, GenBspMeshNode},
        parser::{bsp_file, VisData},
    },
    shader::{
        lexer::{LexedShader, LexedShaderGlobalParam, Map, ShaderStageParam},
        loader::{
            strip_extension, LexedShaderCache, LoadedShader, LoadedShaderCache, TextureCache,
            TryEverythingTextureLoader,
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

#[derive(Resource, Asset)]
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

#[derive(Resource, Default)]
pub struct MeshBoxedLoader;

impl Loader<Mesh> for MeshBoxedLoader {
    fn load(
        &self,
        url: LoadSource,
        _fs: &Filesystem,
        _load_queues: &AssetLoadQueues<'_>,
    ) -> Result<Mesh> {
        let LoadSource::BoxedAsset(dyn_asset) = url else {
            return Err(anyhow!("Expected boxed asset"));
        };

        Ok(*dyn_asset
            .downcast()
            .map_err(|_| anyhow!("Failed to downcast LoadSource::BoxedAsset to Mesh"))?)
    }
}

#[derive(Resource, Default)]
pub struct ShaderBoxedLoader;

impl Loader<LoadedShader> for ShaderBoxedLoader {
    fn load(
        &self,
        url: LoadSource,
        _fs: &Filesystem,
        _load_queues: &AssetLoadQueues<'_>,
    ) -> Result<LoadedShader> {
        let LoadSource::BoxedAsset(dyn_asset) = url else {
            return Err(anyhow!("Expected boxed asset"));
        };

        Ok(*dyn_asset
            .downcast()
            .map_err(|_| anyhow!("Failed to downcast LoadSource::BoxedAsset to LoadedShader"))?)
    }
}

#[derive(Default, Resource)]
pub struct BspLoader {
    lexed_shader_cache: Lock<LexedShaderCache>,
    loaded_shader_cache: Lock<LoadedShaderCache>,
    texture_cache: Lock<TextureCache>,
}

impl BspLoader {
    fn load_shader_from_lexed(
        &self,
        shader: LexedShader,
        load_queues: &AssetLoadQueues<'_>,
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
                let mut texture_load_queue = load_queues
                    .get_load_queue::<Texture, TryEverythingTextureLoader>()
                    .unwrap();
                let handle = texture_load_queue.enqueue(LoadSource::Url(Url::new(path)));
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
                            let mut texture_load_queue = load_queues
                                .get_load_queue::<Texture, TryEverythingTextureLoader>()
                                .unwrap();
                            let handle =
                                texture_load_queue.enqueue(LoadSource::Url(Url::new(path)));
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

impl Loader<Bsp> for BspLoader {
    // TODO: clean this up
    fn load(
        &self,
        url: LoadSource,
        fs: &Filesystem,
        load_queues: &AssetLoadQueues<'_>,
    ) -> Result<Bsp> {
        let bytes = fs.read_sub_path(url.as_path().unwrap())?;
        let (_, bsp_file) = bsp_file(bytes.as_slice())
            .finish()
            .map_err(|e| anyhow!("Failed to parse bsp file: {:?}", e.code))?;

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

        let mut mesh_load_queue = load_queues
            .get_load_queue::<Mesh, MeshBoxedLoader>()
            .unwrap();

        let mut shader_load_queue = load_queues
            .get_load_queue::<LoadedShader, ShaderBoxedLoader>()
            .unwrap();

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
                        let mesh = mesh_load_queue.enqueue(mesh);
                        let texture_name = texture.to_str().unwrap();
                        let texture_name = strip_extension(texture_name);

                        let mut loaded_shader_cache = self.loaded_shader_cache.write();
                        let lexed_shader_cache = self.lexed_shader_cache.read();

                        if let Some(shader) = loaded_shader_cache.get(texture_name) {
                            shader_meshes.push(LoadedBspShaderMesh { mesh, shader, typ });
                        } else if let Some(shader) = lexed_shader_cache.get(texture_name) {
                            let shader = self.load_shader_from_lexed(shader.clone(), load_queues);
                            let shader = shader_load_queue.enqueue(shader);
                            loaded_shader_cache.insert(texture_name.to_string(), shader);
                            shader_meshes.push(LoadedBspShaderMesh { mesh, shader, typ });
                        } else {
                            let texture_cache = self.texture_cache.read();
                            if let Some(texture) = texture_cache.get(texture_name) {
                                let shader =
                                    LoadedShader::make_simple_textured(texture, texture_name);
                                let shader = shader_load_queue.enqueue(shader);
                                shader_meshes.push(LoadedBspShaderMesh { mesh, shader, typ });
                            } else {
                                // try to load it again
                                let mut texture_load_queue = load_queues
                                    .get_load_queue::<Texture, TryEverythingTextureLoader>()
                                    .unwrap();
                                let texture = texture_load_queue
                                    .enqueue(LoadSource::Url(Url::new(texture.to_str().unwrap())));
                                let shader =
                                    LoadedShader::make_simple_textured(texture, texture_name);
                                let shader = shader_load_queue.enqueue(shader);
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
