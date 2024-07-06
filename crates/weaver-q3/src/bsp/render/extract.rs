use std::{ops::Range, sync::Arc};

use encase::ShaderType;
use weaver_asset::Assets;
use weaver_core::{
    mesh::{Mesh, Vertex},
    prelude::{Vec2, Vec3},
    texture::Texture,
};
use weaver_ecs::{
    commands::WorldMut,
    component::Res,
    prelude::{Component, Resource},
};
use weaver_renderer::{
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
        lexer::Map,
        loader::LoadedShader,
        render::{ShaderBindGroupLayout, SHADER_TEXTURE_ARRAY_SIZE},
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

/// A group of shaders that have been batched together for a single bind group, pipeline, and draw call.
#[derive(Component)]
pub struct BatchedBspShaderIndices {
    pub textures: Vec<GpuTexture>,
    pub texture_ibo_indices: Vec<u32>,
    pub ibo_range: Range<u32>,
    pub bind_group: wgpu::BindGroup,
    pub sampler: wgpu::Sampler,
    pub dummy_texture: GpuTexture,
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
    pub ibo: wgpu::Buffer,
    pub num_indices: u32,
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

#[allow(clippy::too_many_arguments)]
pub fn extract_bsps(
    mut world: WorldMut,
    bsp: Extract<Res<'static, Bsp>>,
    source_meshes: Extract<Res<Assets<Mesh>>>,
    source_shaders: Extract<Res<'static, Assets<LoadedShader>>>,
    source_textures: Extract<Res<'static, Assets<Texture>>>,
    bind_group_layout: Res<ShaderBindGroupLayout>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) -> Result<()> {
    if !world.has_resource::<ExtractedBsp>() {
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

        let mut shader_mesh_index = 0;
        let shader_meshes = shader_meshes.into_iter().collect::<Vec<_>>();

        let mut vbo = Vec::new();
        let mut ibo = Vec::new();

        let dummy_texture = Texture::from_rgba8(&[255, 0, 255, 255], 1, 1);

        let dummy_texture =
            GpuTexture::from_image(&device, &queue, &dummy_texture, texture_format::SDR_FORMAT)
                .unwrap();

        // batch shaders together
        'batch_outer: loop {
            let mut total_stages = 0;
            let mut outer_textures = Vec::new();
            let mut texture_ibo_indices = Vec::new();

            let ibo_start = ibo.len() as u32;

            'gather_stages: loop {
                if shader_mesh_index >= shader_meshes.len() {
                    break 'batch_outer;
                }
                let (shader, meshes) = shader_meshes.get(shader_mesh_index).unwrap();
                let shader = source_shaders.get(*shader).unwrap();

                if total_stages + shader.shader.stages.len() > SHADER_TEXTURE_ARRAY_SIZE as usize {
                    break 'gather_stages;
                }

                for stage in &shader.shader.stages {
                    if let Some(ref texture) = stage.texture_map() {
                        if *texture == Map::WhiteImage || *texture == Map::Lightmap {
                            continue;
                        }
                        let texture = shader.textures.get(texture).unwrap();
                        let texture = source_textures.get(*texture).unwrap();
                        let texture = GpuTexture::from_image(
                            &device,
                            &queue,
                            &texture,
                            texture_format::SDR_FORMAT,
                        )
                        .unwrap();
                        outer_textures.push(texture);
                    } else {
                        outer_textures.push(dummy_texture.clone());
                    }

                    for mesh in meshes {
                        let mut tmp_vbo = Vec::new();
                        for vertex in &mesh.vertices {
                            let tex_idx = total_stages as u32;
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
                            ibo.push(*index + vbo_offset);
                        }
                    }

                    total_stages += 1;
                }

                shader_mesh_index += 1;
            }

            let ibo_end = ibo.len() as u32;

            if texture_ibo_indices.len() < SHADER_TEXTURE_ARRAY_SIZE as usize * 2 {
                let dummy_indices = vec![
                    u32::MAX;
                    (SHADER_TEXTURE_ARRAY_SIZE as usize * 2)
                        - texture_ibo_indices.len()
                ];
                texture_ibo_indices.extend(dummy_indices);
            }

            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });

            let mut views = outer_textures
                .iter()
                .map(|stage| &*stage.view)
                .collect::<Vec<_>>();

            if views.is_empty() {
                views.push(&dummy_texture.view);
            }

            if views.len() < SHADER_TEXTURE_ARRAY_SIZE as usize {
                let dummy_views = vec![views[0]; SHADER_TEXTURE_ARRAY_SIZE as usize - views.len()];
                views.extend(dummy_views);
            }

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureViewArray(&views),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
                label: Some("BSP Shader Bind Group"),
            });

            let extracted_bsp_shader_indices = BatchedBspShaderIndices {
                textures: outer_textures,
                ibo_range: ibo_start..ibo_end,
                texture_ibo_indices,
                bind_group,
                sampler,
                dummy_texture: dummy_texture.clone(),
            };

            world.spawn(extracted_bsp_shader_indices);
        }

        log::debug!("Extracted BSP");

        let vbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("BSP VBO"),
            contents: bytemuck::cast_slice(&vbo),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let num_indices = ibo.len() as u32;

        let ibo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("BSP IBO"),
            contents: bytemuck::cast_slice(&ibo),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });

        let extracted_bsp = ExtractedBsp {
            nodes,
            vis_data: bsp.vis_data.clone(),
            vbo,
            ibo,
            num_indices,
        };

        world.insert_resource(extracted_bsp);
    }

    Ok(())
}
