use weaver_asset::{prelude::Asset, Assets, Handle};
use weaver_core::mesh::Mesh;
use weaver_ecs::{
    commands::Commands,
    component::{Res, ResMut},
    query::Query,
    system::SystemParamWrapper,
};
use weaver_pbr::material::{GpuMaterial, Material};
use weaver_renderer::{
    asset::{ExtractedRenderAssets, RenderAsset},
    bind_group::{BindGroup, BindGroupLayoutCache, ExtractedAssetBindGroups},
    extract::Extract,
    mesh::GpuMesh,
    WgpuDevice, WgpuQueue,
};
use weaver_util::prelude::Result;

use crate::bsp::{
    generator::{BspFaceType, BspPlane},
    loader::{Bsp, BspNode},
    parser::VisData,
};

#[derive(Debug, Clone)]
pub enum ExtractedBspNode {
    Leaf {
        meshes_and_materials: Vec<(Handle<GpuMesh>, Handle<BindGroup<GpuMaterial>>, BspFaceType)>,
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
    commands: Commands,
    query: Extract<Query<&'static Handle<Bsp>>>,
    source_assets: Extract<Res<'static, Assets<Bsp>>>,
    source_meshes: Extract<Res<Assets<Mesh>>>,
    source_materials: Extract<Res<Assets<Material>>>,
    source_textures: SystemParamWrapper<<GpuMaterial as RenderAsset>::Param>,
    mut render_assets: ResMut<Assets<ExtractedBsp>>,
    mut render_meshes: ResMut<Assets<GpuMesh>>,
    mut render_materials: ResMut<Assets<GpuMaterial>>,
    mut render_bind_groups: ResMut<Assets<BindGroup<GpuMaterial>>>,
    extracted_assets: Res<ExtractedRenderAssets>,
    extracted_bind_groups: Res<ExtractedAssetBindGroups>,
    mut cache: ResMut<BindGroupLayoutCache>,
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
                            meshes_and_materials,
                            parent,
                            cluster,
                        } => {
                            let mut extracted_meshes_and_materials = Vec::new();
                            for (mesh, material, typ) in meshes_and_materials {
                                let source_mesh = source_meshes.get(*mesh).unwrap();
                                let source_material = source_materials.get(*material).unwrap();

                                let extracted_mesh = GpuMesh::extract_render_asset(
                                    &source_mesh,
                                    &(),
                                    &device,
                                    &queue,
                                )
                                .unwrap();
                                let extracted_material = GpuMaterial::extract_render_asset(
                                    &source_material,
                                    source_textures.item(),
                                    &device,
                                    &queue,
                                )
                                .unwrap();

                                let extracted_mesh = render_meshes.insert(extracted_mesh);

                                let material_bind_group = BindGroup::<GpuMaterial>::new(
                                    &device,
                                    &extracted_material,
                                    &mut cache,
                                );
                                let extracted_material =
                                    render_materials.insert(extracted_material);
                                let extracted_material_bind_group =
                                    render_bind_groups.insert(material_bind_group);

                                extracted_assets
                                    .insert(mesh.into_untyped(), extracted_mesh.into_untyped());
                                extracted_assets.insert(
                                    material.into_untyped(),
                                    extracted_material.into_untyped(),
                                );
                                extracted_bind_groups.insert(
                                    material.into_untyped(),
                                    extracted_material_bind_group.into_untyped(),
                                );

                                extracted_meshes_and_materials.push((
                                    extracted_mesh,
                                    extracted_material_bind_group,
                                    *typ,
                                ));
                            }
                            extracted_bsp.insert(
                                i,
                                ExtractedBspNode::Leaf {
                                    meshes_and_materials: extracted_meshes_and_materials,
                                    parent: *parent,
                                    cluster: *cluster,
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
            commands.insert_component(entity, render_handle);
        }
    }

    Ok(())
}
