use std::fmt::Debug;

use encase::ShaderType;
use weaver_asset::Assets;
use weaver_core::{
    mesh::Mesh,
    prelude::{Vec2, Vec3},
    texture::Texture,
};
use weaver_ecs::{
    component::{Res, ResMut},
    prelude::{Component, Resource},
    world::World,
};
use weaver_renderer::{
    bind_group::BindGroupLayoutCache,
    extract::Extract,
    prelude::wgpu,
    texture::{texture_format, GpuTexture},
    WgpuDevice, WgpuQueue,
};
use weaver_util::prelude::{FxHashMap, Result};
use wgpu::util::DeviceExt;

use crate::{
    bsp::{
        generator::BspPlane,
        loader::{Bsp, BspNode, LoadedBspShaderMesh},
        parser::VisData,
    },
    shader::{
        loader::LoadedShader,
        render::{
            ShaderBindGroupLayout, ShaderPipeline, ShaderPipelineCache, ShaderPipelineKey,
            SHADER_TEXTURE_ARRAY_SIZE,
        },
    },
};

#[derive(Clone, Copy, Debug, ShaderType, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct VertexWithTexIdx {
    pub position: Vec3,
    pub normal: Vec3,
    pub tangent: Vec3,
    pub tex_coords: Vec2,
    pub tex_idx: u32,
}

#[derive(Debug, Clone, Component)]
pub enum ExtractedBspNode {
    Leaf {
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

impl ExtractedBspNode {
    pub fn parent(&self) -> Option<usize> {
        match self {
            ExtractedBspNode::Leaf { parent, .. } => Some(*parent),
            ExtractedBspNode::Node { parent, .. } => *parent,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WalkDirection {
    Back,
    Front,
    Skip,
}

#[derive(Resource)]
pub struct ExtractedBsp {
    pub nodes: Vec<Option<ExtractedBspNode>>,
    pub vis_data: VisData,
    pub vbo: wgpu::Buffer,
    pub key_paths: ShaderKeyPath,
}

impl ExtractedBsp {
    pub const fn root(&self) -> usize {
        0
    }

    pub fn insert(&mut self, index: usize, node: ExtractedBspNode) {
        self.nodes[index] = Some(node);
    }

    pub fn node_iter(&self) -> impl Iterator<Item = (usize, &ExtractedBspNode)> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(i, node)| node.as_ref().map(|n| (i, n)))
    }

    pub fn nodes_sorted_by<F>(&self, mut f: F) -> Vec<(usize, &ExtractedBspNode)>
    where
        F: FnMut(&ExtractedBspNode, &ExtractedBspNode) -> std::cmp::Ordering,
    {
        let mut nodes = self.node_iter().collect::<Vec<_>>();
        nodes.sort_unstable_by(|(_, a), (_, b)| f(a, b));
        nodes
    }

    pub fn walk<F>(&self, index: usize, visitor: &mut F)
    where
        F: FnMut(usize, &ExtractedBspNode),
    {
        let mut stack = vec![index];
        while let Some(index) = stack.pop() {
            if let Some(node) = &self.nodes[index] {
                visitor(index, node);
                match node {
                    ExtractedBspNode::Leaf { .. } => {}
                    ExtractedBspNode::Node { back, front, .. } => {
                        stack.push(*front);
                        stack.push(*back);
                    }
                }
            }
        }
    }

    pub fn walk_directed<F>(&self, index: usize, visitor: &mut F)
    where
        F: FnMut(usize, &ExtractedBspNode) -> WalkDirection,
    {
        let mut stack = vec![index];
        while let Some(index) = stack.pop() {
            if let Some(node) = &self.nodes[index] {
                let direction = visitor(index, node);
                match node {
                    ExtractedBspNode::Leaf { .. } => {}
                    ExtractedBspNode::Node { back, front, .. } => match direction {
                        WalkDirection::Back => stack.push(*back),
                        WalkDirection::Front => stack.push(*front),
                        WalkDirection::Skip => {}
                    },
                }
            }
        }
    }
}

/// A tree of possible shader key paths.
///
/// In other words, this is a tree of all possible combinations of shader keys that can be used in various sequences in a BSP map.
///
/// For example, if a shader uses key A in its first stage, then key B in its second stage, then key C in its third stage,
/// then the tree would look like this:
///
/// ```text
/// root
/// ├── A
/// │   └── B
/// │       └── C
/// ```
///
/// If another shader used key A, then key B, then key D, then the tree would look like this:
///
/// ```text
/// root
/// ├── A
/// │   └── B
/// │       ├── C
/// │       └── D
/// ```
///
/// This tree can be used to determine which shaders can be batched together based on their stages.
/// Each node of the tree corresponds to one batch, and each batch will result in one draw call.
///
pub struct ShaderKeyPath {
    pub tree: FxHashMap<ShaderPipelineKey, ShaderKeyPath>,
    pub stages: Vec<BatchedShaderStages>,
}

impl ShaderKeyPath {
    pub fn walk<'a, F>(&'a self, f: &mut F)
    where
        F: FnMut(&'a BatchedShaderStages),
    {
        for stage in &self.stages {
            f(stage);
        }
        for (_, child) in self.tree.iter() {
            child.walk(f);
        }
    }

    pub fn node_count(&self) -> usize {
        let mut count = 1;
        for (_, child) in self.tree.iter() {
            count += child.node_count();
        }
        count
    }

    pub fn batch_count(&self) -> usize {
        let mut count = self.stages.len();
        for (_, child) in self.tree.iter() {
            count += child.batch_count();
        }
        count
    }
}

impl Debug for ShaderKeyPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.tree.iter()).finish()
    }
}

pub struct BatchedShaderStages {
    pub key: ShaderPipelineKey,
    pub textures: Vec<GpuTexture>,
    pub indices: Vec<u32>,
    pub index_buffer: Option<wgpu::Buffer>,
    pub num_indices: u32,
    pub bind_group: Option<wgpu::BindGroup>,
    pub sampler: wgpu::Sampler,
    pub dummy_texture: GpuTexture,
}

#[allow(clippy::too_many_arguments)]
pub fn extract_bsps(
    world: &mut World,
    bsp: Extract<Res<'static, Bsp>>,
    source_meshes: Extract<Res<Assets<Mesh>>>,
    source_shaders: Extract<Res<'static, Assets<LoadedShader>>>,
    source_textures: Extract<Res<'static, Assets<Texture>>>,
    bind_group_layout: Res<ShaderBindGroupLayout>,
    mut pipeline_cache: ResMut<ShaderPipelineCache>,
    mut bind_group_layout_cache: ResMut<BindGroupLayoutCache>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) -> Result<()> {
    if world.has_resource::<ExtractedBsp>() {
        return Ok(());
    }

    let mut nodes = vec![None; bsp.nodes.len()];
    let mut shader_meshes = FxHashMap::default();

    for (i, node) in bsp.nodes.iter().enumerate() {
        if let Some(node) = node {
            match node {
                BspNode::Leaf {
                    shader_meshes: leaf_shader_meshes,
                    parent,
                    cluster,
                    min,
                    max,
                } => {
                    for loaded_mesh in leaf_shader_meshes {
                        let LoadedBspShaderMesh { mesh, shader, .. } = loaded_mesh;
                        let source_mesh = source_meshes.get(*mesh).unwrap();
                        shader_meshes
                            .entry(*shader)
                            .or_insert_with(Vec::new)
                            .push(source_mesh.clone());
                    }
                    nodes[i] = Some(ExtractedBspNode::Leaf {
                        parent: *parent,
                        cluster: *cluster,
                        min: *min,
                        max: *max,
                    });
                }
                BspNode::Node {
                    plane,
                    back,
                    front,
                    parent,
                    ..
                } => {
                    nodes[i] = Some(ExtractedBspNode::Node {
                        plane: *plane,
                        back: *back,
                        front: *front,
                        parent: *parent,
                    });
                }
            }
        }
    }

    let mut vbo = Vec::new();

    let dummy_texture = Texture::from_rgba8(&[255, 255, 255, 255], 1, 1);

    let dummy_texture =
        GpuTexture::from_image(&device, &queue, &dummy_texture, texture_format::SDR_FORMAT)
            .unwrap();

    // generate the tree of possible key paths
    let mut key_paths = ShaderKeyPath {
        tree: FxHashMap::default(),
        stages: Vec::new(),
    };
    for (shader, meshes) in &shader_meshes {
        let shader = source_shaders.get(*shader).unwrap();

        let mut current = &mut key_paths;

        for stage in shader.shader.stages.iter() {
            let (texture, is_dummy) = if let Some(ref texture) = stage.texture_map() {
                // if *texture == Map::WhiteImage || *texture == Map::Lightmap {
                //     continue;
                // }

                let texture = shader.textures.get(texture).unwrap();
                let texture = source_textures.get(*texture).unwrap();
                let texture =
                    GpuTexture::from_image(&device, &queue, &texture, texture_format::SDR_FORMAT)
                        .unwrap();
                (texture, false)
            } else {
                (dummy_texture.clone(), true)
            };

            let key = ShaderPipelineKey {
                blend_func: stage.blend_func().copied(),
                cull: shader.shader.cull(),
            };

            let entry = current.tree.entry(key).or_insert_with(|| {
                let graph = FxHashMap::default();
                ShaderKeyPath {
                    tree: graph,
                    stages: Vec::new(),
                }
            });

            let mut need_to_create_new_batch = true;

            if let Some(last) = entry.stages.last() {
                assert_eq!(last.key, key);
                need_to_create_new_batch = last.textures.len() + shader.shader.stages.len()
                    >= SHADER_TEXTURE_ARRAY_SIZE as usize;
            }

            if need_to_create_new_batch {
                let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("BSP Sampler"),
                    address_mode_u: wgpu::AddressMode::Repeat,
                    address_mode_v: wgpu::AddressMode::Repeat,
                    address_mode_w: wgpu::AddressMode::Repeat,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::FilterMode::Linear,
                    ..Default::default()
                });

                let dummy_texture = dummy_texture.clone();

                entry.stages.push(BatchedShaderStages {
                    key,
                    textures: Vec::new(),
                    indices: Vec::new(),
                    index_buffer: None,
                    num_indices: 0,
                    bind_group: None,
                    sampler,
                    dummy_texture,
                });
            }

            let current_batch = entry.stages.last_mut().unwrap();

            let tex_idx = if is_dummy {
                u32::MAX
            } else {
                current_batch.textures.len() as u32
            };

            current_batch.textures.push(texture);

            for mesh in meshes {
                let mut tmp_vbo = Vec::new();
                for vertex in &mesh.vertices {
                    let vertex = VertexWithTexIdx {
                        position: vertex.position,
                        normal: vertex.normal,
                        tangent: vertex.tangent,
                        tex_coords: vertex.tex_coords,
                        tex_idx,
                    };
                    tmp_vbo.push(vertex);
                }

                let vbo_offset = vbo.len() as u32;
                vbo.extend(tmp_vbo);

                for index in &mesh.indices {
                    current_batch.indices.push(*index + vbo_offset);
                }
            }

            current_batch.num_indices = current_batch.indices.len() as u32;

            current = entry;
        }
    }

    // dbg!(key_paths.node_count(), key_paths.batch_count());

    // recursively generate the batch data for each key path
    recursively_generate_batch_data(
        &mut key_paths,
        &device,
        &bind_group_layout,
        &mut pipeline_cache,
        &mut bind_group_layout_cache,
    );

    log::debug!("Extracted BSP");

    let vbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("BSP VBO"),
        contents: bytemuck::cast_slice(&vbo),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });

    let extracted_bsp = ExtractedBsp {
        nodes,
        vis_data: bsp.vis_data.clone(),
        vbo,
        key_paths,
    };

    world.insert_resource(extracted_bsp);

    Ok(())
}

fn recursively_generate_batch_data(
    tree: &mut ShaderKeyPath,
    device: &wgpu::Device,
    bind_group_layout: &ShaderBindGroupLayout,
    pipeline_cache: &mut ShaderPipelineCache,
    bind_group_layout_cache: &mut BindGroupLayoutCache,
) {
    for stages in &mut tree.stages {
        stages.index_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("BSP IBO"),
                contents: bytemuck::cast_slice(&stages.indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            }),
        );

        let mut views = stages
            .textures
            .iter()
            .map(|stage| &*stage.view)
            .collect::<Vec<_>>();

        if views.is_empty() {
            views.push(&stages.dummy_texture.view);
        }

        if views.len() < SHADER_TEXTURE_ARRAY_SIZE as usize {
            let dummy_views = vec![views[0]; SHADER_TEXTURE_ARRAY_SIZE as usize - views.len()];
            views.extend(dummy_views);
        }

        stages.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureViewArray(&views),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&stages.sampler),
                },
            ],
            label: Some("BSP Shader Bind Group"),
        }));

        stages.num_indices = stages.indices.len() as u32;

        let key = stages.key;

        pipeline_cache.cache.entry(key).or_insert_with(|| {
            ShaderPipeline::from_key(key, device, bind_group_layout, bind_group_layout_cache)
        });
    }

    for (_, child) in tree.tree.iter_mut() {
        recursively_generate_batch_data(
            child,
            device,
            bind_group_layout,
            pipeline_cache,
            bind_group_layout_cache,
        );
    }
}
