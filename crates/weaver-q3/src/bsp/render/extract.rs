use std::sync::Arc;

use weaver_asset::{Assets, Handle};
use weaver_core::{
    mesh::{Mesh, Vertex},
    prelude::Vec3,
};
use weaver_ecs::{
    commands::WorldMut,
    component::{Res, ResMut},
    prelude::{Component, Resource},
    system::SystemParamWrapper,
};
use weaver_renderer::{
    asset::{ExtractedRenderAssets, RenderAsset},
    extract::Extract,
    prelude::wgpu,
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
    shader::{loader::LoadedShader, render::extract::ExtractedShader},
};

// #[derive(Debug, Clone)]
// pub struct ExtractedBspMesh {
//     pub mesh: Handle<Mesh>,
//     pub shader: Handle<ExtractedShader>,
//     pub typ: BspFaceType,
// }

#[derive(Debug, Clone)]
pub struct IndexBuffer {
    pub buffer: Arc<wgpu::Buffer>,
    pub num_indices: u32,
}

#[derive(Debug, Clone, Component)]
pub struct ExtractedBspShaderIndices {
    pub shader: Handle<ExtractedShader>,
    pub vbo_indices: IndexBuffer,
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
    mut shader_param: SystemParamWrapper<<ExtractedShader as RenderAsset>::Param>,
    mut render_shaders: ResMut<Assets<ExtractedShader>>,
    extracted_assets: Res<ExtractedRenderAssets>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) -> Result<()> {
    if !world.has_resource::<ExtractedBsp>() {
        let mut nodes = vec![None; bsp.nodes.len()];
        // let mut extracted_bsp = ExtractedBsp::with_capacity(bsp.nodes.len(), bsp.vis_data.clone());
        let mut vbo: Vec<Vertex> = Vec::new();
        let mut shader_mesh_indices = FxHashMap::default();
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

                            let base_index = vbo.len() as u32;
                            vbo.extend(&source_mesh.vertices);
                            let indices = source_mesh
                                .indices
                                .iter()
                                .map(|i| i + base_index)
                                .collect::<Vec<_>>();

                            shader_mesh_indices
                                .entry(*shader)
                                .or_insert_with(Vec::new)
                                .extend(indices);
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

        for (shader, vbo_indices) in shader_mesh_indices {
            let extracted_shader = source_shaders.get(shader).unwrap();
            let extracted_shader = ExtractedShader::extract_render_asset(
                &extracted_shader,
                shader_param.item_mut(),
                &device,
                &queue,
            )
            .unwrap();

            let extracted_shader_handle = render_shaders.insert(extracted_shader);

            extracted_assets.insert(
                shader.into_untyped(),
                extracted_shader_handle.into_untyped(),
            );

            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("BSP Index Buffer"),
                contents: bytemuck::cast_slice(&vbo_indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            });

            let vbo_indices = IndexBuffer {
                buffer: Arc::new(index_buffer),
                num_indices: vbo_indices.len() as u32,
            };

            let extracted_bsp_shader_indices = ExtractedBspShaderIndices {
                shader: extracted_shader_handle,
                vbo_indices,
            };

            world.spawn(extracted_bsp_shader_indices);
        }

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
        };

        world.insert_resource(extracted_bsp);
    }

    Ok(())
}
