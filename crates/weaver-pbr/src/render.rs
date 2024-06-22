use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    path::Path,
};

use weaver_asset::{Assets, Handle};
use weaver_core::{prelude::Mat4, transform::Transform};
use weaver_ecs::{
    component::Res,
    entity::Entity,
    prelude::{Resource, World},
    storage::Ref,
    world::WorldLock,
};
use weaver_renderer::{
    bind_group::{BindGroup, BindGroupLayoutCache},
    camera::{GpuCamera, ViewTarget},
    graph::{RenderCtx, RenderGraphCtx, ViewNode},
    hdr::HdrRenderTarget,
    mesh::GpuMesh,
    pipeline::{CreateRenderPipeline, RenderPipeline, RenderPipelineCache, RenderPipelineLayout},
    prelude::*,
    render_command::RenderCommand,
    render_phase::{BatchedInstanceBuffer, BinnedRenderPhases, GetBatchData},
    shader::Shader,
    texture::texture_format,
    RenderLabel,
};
use weaver_util::prelude::Result;

use crate::{light::GpuPointLightArray, material::GpuMaterial, PbrDrawItem};

pub struct PbrMeshInstance {
    pub mesh: Handle<GpuMesh>,
    pub material: Handle<BindGroup<GpuMaterial>>,
    pub transform: Transform,
}

#[derive(Resource, Default)]
pub struct PbrMeshInstances(HashMap<Entity, PbrMeshInstance>);

impl Deref for PbrMeshInstances {
    type Target = HashMap<Entity, PbrMeshInstance>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PbrMeshInstances {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl GetBatchData for PbrMeshInstances {
    type Param = Res<PbrMeshInstances>;
    type BufferData = Mat4;
    type UpdateQuery = (
        &'static Handle<GpuMesh>,
        &'static Handle<BindGroup<GpuMaterial>>,
        &'static Transform,
    );

    fn update_from_world(&mut self, render_world: &World) {
        self.0.clear();
        let query = render_world.query::<(
            &Handle<GpuMesh>,
            &Handle<BindGroup<GpuMaterial>>,
            &Transform,
        )>();
        for (entity, (mesh, material, transform)) in query.iter() {
            self.0.insert(
                entity,
                PbrMeshInstance {
                    mesh: *mesh,
                    material: *material,
                    transform: *transform,
                },
            );
        }
    }

    fn get_batch_data(param: &Self::Param, query_item: Entity) -> Option<Self::BufferData> {
        param
            .get(&query_item)
            .map(|instance| instance.transform.matrix())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PbrNodeLabel;
impl RenderLabel for PbrNodeLabel {}

#[derive(Default)]
pub struct PbrNode;

impl CreateRenderPipeline for PbrNode {
    fn create_render_pipeline_layout(
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> RenderPipelineLayout
    where
        Self: Sized,
    {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PBR Pipeline Layout"),
            bind_group_layouts: &[
                &bind_group_layout_cache.get_or_create::<GpuMaterial>(device),
                &bind_group_layout_cache.get_or_create::<GpuCamera>(device),
                &bind_group_layout_cache
                    .get_or_create::<BatchedInstanceBuffer<PbrDrawItem, PbrRenderCommand>>(device),
                &bind_group_layout_cache.get_or_create::<GpuPointLightArray>(device),
            ],
            push_constant_ranges: &[],
        });

        RenderPipelineLayout::new(layout)
    }

    fn create_render_pipeline(
        device: &wgpu::Device,
        cached_layout: &wgpu::PipelineLayout,
    ) -> RenderPipeline
    where
        Self: Sized,
    {
        let shader = Shader::new(Path::new("assets/shaders/pbr.wgsl")).create_shader_module(device);

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PBR Pipeline"),
            layout: Some(cached_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 4 * (3 + 3 + 3 + 2) as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 4 * 3,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 4 * 6,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 4 * 9,
                            shader_location: 3,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture_format::HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture_format::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: Default::default(),
            multiview: None,
        });

        RenderPipeline::new(pipeline)
    }
}

impl ViewNode for PbrNode {
    type ViewQueryFetch = &'static ViewTarget;
    type ViewQueryFilter = ();

    fn run(
        &self,
        render_world: &WorldLock,
        graph_ctx: &mut RenderGraphCtx,
        render_ctx: &mut RenderCtx,
        _view_target: &Ref<ViewTarget>,
    ) -> Result<()> {
        let Some(binned_phases) = render_world.get_resource::<BinnedRenderPhases<PbrDrawItem>>()
        else {
            return Ok(());
        };

        let Some(phase) = binned_phases.get(&graph_ctx.view_entity) else {
            return Ok(());
        };

        let Some(draw_functions) = render_world.get_resource::<DrawFunctions<PbrDrawItem>>() else {
            return Ok(());
        };
        let mut draw_functions = draw_functions.write();
        draw_functions.prepare(render_world).unwrap();

        {
            if !phase.is_empty() {
                phase
                    .render(
                        render_world,
                        render_ctx.command_encoder(),
                        graph_ctx.view_entity,
                        &mut draw_functions,
                    )
                    .unwrap();
            }
        }

        Ok(())
    }
}

pub struct PbrRenderCommand;

impl RenderCommand<PbrDrawItem> for PbrRenderCommand {
    type Param = (
        Res<Assets>,
        Res<RenderPipelineCache>,
        Res<BindGroup<BatchedInstanceBuffer<PbrDrawItem, PbrRenderCommand>>>,
        Res<BindGroup<GpuPointLightArray>>,
        Res<HdrRenderTarget>,
    );

    type ViewQueryFetch = (&'static ViewTarget, &'static BindGroup<GpuCamera>);

    type ViewQueryFilter = ();

    type ItemQueryFetch = ();

    type ItemQueryFilter = ();

    fn render(
        item: &PbrDrawItem,
        view_query: <Self::ViewQueryFetch as weaver_ecs::prelude::QueryFetch>::Fetch,
        _item_query: Option<<Self::ItemQueryFetch as weaver_ecs::prelude::QueryFetch>::Fetch>,
        param: Self::Param,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<()> {
        let (assets, pipeline_cache, mesh_transforms_bind_group, lights_bind_group, hdr_target) =
            param;
        let (view_target, camera_bind_group) = view_query;
        let mesh = assets.get::<GpuMesh>(item.key.mesh).unwrap();
        let material_bind_group = assets
            .get::<BindGroup<GpuMaterial>>(item.key.material)
            .unwrap();
        let pipeline = pipeline_cache.get_pipeline::<PbrNode>().unwrap();

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("PBR Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: hdr_target.color_target(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &view_target.depth_target,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, material_bind_group.bind_group(), &[]);
        pass.set_bind_group(1, camera_bind_group.bind_group(), &[]);
        pass.set_bind_group(2, mesh_transforms_bind_group.bind_group(), &[]);
        pass.set_bind_group(3, lights_bind_group.bind_group(), &[]);

        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.num_indices, 0, item.batch_range.clone());

        Ok(())
    }
}
