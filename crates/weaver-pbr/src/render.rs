use std::{
    ops::{Deref, DerefMut},
    path::Path,
};

use weaver_asset::{Assets, Handle};
use weaver_core::{prelude::Mat4, transform::Transform};
use weaver_ecs::{
    component::Res,
    entity::{Entity, EntityMap},
    prelude::{QueryFetch, Resource, World, WorldView},
    storage::Ref,
    system::SystemParamItem,
    world::FromWorld,
};
use weaver_renderer::{
    bind_group::{BindGroup, BindGroupLayout, BindGroupLayoutCache, CreateBindGroup},
    camera::{CameraBindGroup, ViewTarget},
    graph::{RenderCtx, RenderGraphCtx, ViewNode},
    mesh::GpuMesh,
    pipeline::{CreateRenderPipeline, RenderPipeline, RenderPipelineCache, RenderPipelineLayout},
    prelude::*,
    render_command::RenderCommand,
    render_phase::{BatchedInstanceBuffer, BinnedRenderPhases, GetBatchData},
    shader::Shader,
    texture::texture_format,
    RenderLabel,
};
use weaver_util::Result;

use crate::{
    light::GpuPointLightArray, material::GpuMaterial, prelude::irradiance::GpuSkyboxIrradiance,
    PbrDrawItem,
};

pub struct PbrMeshInstance {
    pub mesh: Handle<GpuMesh>,
    pub material: Handle<BindGroup<GpuMaterial>>,
    pub transform: Transform,
}

#[derive(Resource, Default)]
pub struct PbrMeshInstances(EntityMap<PbrMeshInstance>);

impl Deref for PbrMeshInstances {
    type Target = EntityMap<PbrMeshInstance>;

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
    type BufferData = Mat4;
    type UpdateQueryFetch = (
        &'static Handle<GpuMesh>,
        &'static Handle<BindGroup<GpuMaterial>>,
        &'static Transform,
    );
    type UpdateQueryFilter = ();

    fn update(&mut self, query: WorldView<Self::UpdateQueryFetch>) {
        self.0.clear();
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

    fn get_batch_data(&self, query_item: Entity) -> Option<Self::BufferData> {
        self.get(&query_item)
            .map(|instance| instance.transform.matrix())
    }
}

/// Combined bind group for PBR light arrays and environment maps.
#[derive(Resource)]
pub struct PbrLightingInformation {
    pub point_lights: GpuPointLightArray,
    pub env_map: GpuSkyboxIrradiance,
}

impl FromWorld for PbrLightingInformation {
    fn from_world(world: &mut World) -> Self {
        let point_lights = GpuPointLightArray::from_world(world);
        let env_map = GpuSkyboxIrradiance::from_world(world);
        Self {
            point_lights,
            env_map,
        }
    }
}

impl CreateBindGroup for PbrLightingInformation {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PBR Lighting Information Bind Group Layout"),
            entries: &[
                // Point lights
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Environment map diffuse
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                // Environment map specular
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                // BRDF LUT
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Environment map sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        cached_layout: &BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PBR Lighting Information Bind Group"),
            layout: cached_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.point_lights.buffer.binding().unwrap(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.env_map.diffuse_cube_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.env_map.specular_cube_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.env_map.brdf_lut_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.env_map.sampler),
                },
            ],
        })
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
                &bind_group_layout_cache.get_or_create::<CameraBindGroup>(device),
                &bind_group_layout_cache
                    .get_or_create::<BatchedInstanceBuffer<PbrDrawItem, PbrRenderCommand>>(device),
                &bind_group_layout_cache.get_or_create::<PbrLightingInformation>(device),
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
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                // cull_mode: None,
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
    type Param = (
        Res<'static, BinnedRenderPhases<PbrDrawItem>>,
        Res<'static, DrawFunctions<PbrDrawItem>>,
    );
    type ViewQueryFetch = &'static ViewTarget;
    type ViewQueryFilter = ();

    fn prepare(&mut self, render_world: &mut World) -> Result<()> {
        let draw_fns = render_world
            .remove_resource::<DrawFunctions<PbrDrawItem>>()
            .unwrap();
        let mut draw_fns_lock = draw_fns.write();
        draw_fns_lock.prepare(render_world).unwrap();
        drop(draw_fns_lock);
        render_world.insert_resource(draw_fns);
        Ok(())
    }

    fn run(
        &self,
        render_world: &World,
        graph_ctx: &mut RenderGraphCtx,
        render_ctx: &mut RenderCtx,
        (binned_phases, draw_functions): &SystemParamItem<Self::Param>,
        view_target: &Ref<ViewTarget>,
    ) -> Result<()> {
        let Some(phase) = binned_phases.get(&graph_ctx.view_entity) else {
            log::debug!(
                "No PBR phase found for view entity {:?}",
                graph_ctx.view_entity
            );
            return Ok(());
        };

        let mut draw_functions_lock = draw_functions.write();

        if !phase.is_empty() {
            let encoder = render_ctx.command_encoder();
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PBR Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view_target.color_target,
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

            log::trace!(
                "Rendering PBR phase for view entity {:?}",
                graph_ctx.view_entity
            );

            phase.render(
                render_world,
                &mut render_pass,
                graph_ctx.view_entity,
                &mut draw_functions_lock,
            )?;
        } else {
            log::trace!(
                "Skipping PBR phase for view entity {:?} because it is empty",
                graph_ctx.view_entity
            );
        }

        Ok(())
    }
}

pub struct PbrRenderCommand;

impl RenderCommand<PbrDrawItem> for PbrRenderCommand {
    type Param = (
        Res<'static, Assets<GpuMesh>>,
        Res<'static, Assets<BindGroup<GpuMaterial>>>,
        Res<'static, RenderPipelineCache>,
        Res<'static, BindGroup<BatchedInstanceBuffer<PbrDrawItem, PbrRenderCommand>>>,
        Res<'static, BindGroup<PbrLightingInformation>>,
    );

    type ViewQueryFetch = &'static BindGroup<CameraBindGroup>;

    type ViewQueryFilter = ();

    type ItemQueryFetch = ();

    type ItemQueryFilter = ();

    fn render<'w>(
        item: PbrDrawItem,
        view_query: <Self::ViewQueryFetch as QueryFetch>::Item<'w>,
        _item_query: Option<<Self::ItemQueryFetch as QueryFetch>::Item<'w>>,
        param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut wgpu::RenderPass<'w>,
    ) -> Result<()> {
        let (
            mesh_assets,
            material_assets,
            pipeline_cache,
            mesh_transforms_bind_group,
            lights_bind_group,
        ) = param;
        let mesh_assets = mesh_assets.into_inner();
        let material_assets = material_assets.into_inner();
        let pipeline_cache = pipeline_cache.into_inner();
        let mesh_transforms_bind_group = mesh_transforms_bind_group.into_inner();
        let lights_bind_group = lights_bind_group.into_inner();

        let camera_bind_group = view_query;
        let camera_bind_group = camera_bind_group.into_inner();

        let mesh = mesh_assets.get(item.key.mesh).unwrap();
        let mesh = mesh.into_inner();

        let material_bind_group = material_assets.get(item.key.material).unwrap();
        let material_bind_group = material_bind_group.into_inner();

        let pipeline = pipeline_cache.get_pipeline_for::<PbrNode>().unwrap();

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
