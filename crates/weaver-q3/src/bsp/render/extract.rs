use weaver_asset::{prelude::Asset, Assets, Handle};
use weaver_core::{mesh::Mesh, prelude::Vec3};
use weaver_ecs::{
    commands::WorldMut,
    component::{Res, ResMut},
    entity::Entity,
    query::Query,
    system::SystemParamWrapper,
};
use weaver_renderer::{
    asset::{ExtractedRenderAssets, RenderAsset},
    extract::Extract,
    mesh::GpuMesh,
    WgpuDevice, WgpuQueue,
};
use weaver_util::prelude::Result;

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
pub enum ExtractedBspNode {
    Leaf {
        shader_mesh_entities: Vec<Entity>,
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

#[derive(Asset)]
pub struct ExtractedBsp {
    pub nodes: Vec<Option<ExtractedBspNode>>,
    pub vis_data: VisData,
}

impl ExtractedBsp {
    pub fn with_capacity(capacity: usize, vis_data: VisData) -> Self {
        Self {
            nodes: vec![None; capacity],
            vis_data,
        }
    }

    pub const fn root(&self) -> usize {
        0
    }

    pub fn insert(&mut self, index: usize, node: ExtractedBspNode) {
        self.nodes[index] = Some(node);
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
    query: Extract<Query<&'static Handle<Bsp>>>,
    source_assets: Extract<Res<'static, Assets<Bsp>>>,
    source_meshes: Extract<Res<Assets<Mesh>>>,
    source_shaders: Extract<Res<'static, Assets<LoadedShader>>>,
    mut shader_param: SystemParamWrapper<<ExtractedShader as RenderAsset>::Param>,
    mut render_assets: ResMut<Assets<ExtractedBsp>>,
    mut render_meshes: ResMut<Assets<GpuMesh>>,
    mut render_shaders: ResMut<Assets<ExtractedShader>>,
    extracted_assets: Res<ExtractedRenderAssets>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) -> Result<()> {
    for (entity, bsp_handle) in query.iter() {
        if !extracted_assets.contains(&bsp_handle.into_untyped()) {
            let bsp = source_assets.get(*bsp_handle).unwrap();

            let mut extracted_bsp =
                ExtractedBsp::with_capacity(bsp.nodes.len(), bsp.vis_data.clone());
            for (i, node) in bsp.nodes.iter().enumerate() {
                if let Some(node) = node {
                    match node {
                        BspNode::Leaf {
                            shader_meshes: meshes_and_materials,
                            parent,
                            cluster,
                            min,
                            max,
                        } => {
                            let mut shader_mesh_entities = Vec::new();
                            for loaded_mesh in meshes_and_materials {
                                let LoadedBspShaderMesh { mesh, shader, typ } = loaded_mesh;
                                let source_mesh = source_meshes.get(*mesh).unwrap();

                                let extracted_mesh = GpuMesh::extract_render_asset(
                                    &source_mesh,
                                    &mut (),
                                    &device,
                                    &queue,
                                )
                                .unwrap();

                                let extracted_mesh = render_meshes.insert(extracted_mesh);

                                extracted_assets
                                    .insert(mesh.into_untyped(), extracted_mesh.into_untyped());

                                let extracted_shader = source_shaders.get(*shader).unwrap();
                                let extracted_shader = ExtractedShader::extract_render_asset(
                                    &extracted_shader,
                                    shader_param.item_mut(),
                                    &device,
                                    &queue,
                                )
                                .unwrap();

                                let extracted_shader_handle =
                                    render_shaders.insert(extracted_shader);

                                extracted_assets.insert(
                                    shader.into_untyped(),
                                    extracted_shader_handle.into_untyped(),
                                );

                                let entity =
                                    world.spawn((extracted_mesh, extracted_shader_handle, *typ));

                                shader_mesh_entities.push(entity);
                            }
                            extracted_bsp.insert(
                                i,
                                ExtractedBspNode::Leaf {
                                    shader_mesh_entities,
                                    parent: *parent,
                                    cluster: *cluster,
                                    min: *min,
                                    max: *max,
                                },
                            );
                        }
                        BspNode::Node {
                            plane,
                            back,
                            front,
                            parent,
                        } => {
                            extracted_bsp.insert(
                                i,
                                ExtractedBspNode::Node {
                                    plane: *plane,
                                    back: *back,
                                    front: *front,
                                    parent: *parent,
                                },
                            );
                        }
                    }
                }
            }

            log::debug!("Extracted BSP");

            let render_handle = render_assets.insert(extracted_bsp);
            extracted_assets.insert(bsp_handle.into_untyped(), render_handle.into_untyped());
            world.insert_component(entity, render_handle);
        }
    }

    Ok(())
}
