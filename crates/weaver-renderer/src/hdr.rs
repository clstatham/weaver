use std::{path::Path, sync::Arc};

use weaver_app::{plugin::Plugin, App};
use weaver_ecs::{
    component::Res,
    prelude::{Resource, World},
    storage::Ref,
    system::SystemParamItem,
    world::FromWorld,
};
use weaver_util::Result;
use weaver_winit::WindowSize;

use crate::{
    bind_group::{BindGroup, BindGroupLayoutCache, CreateBindGroup, ResourceBindGroupPlugin},
    camera::ViewTarget,
    graph::{RenderGraphApp, ViewNode, ViewNodeRunner},
    pipeline::{
        CreateRenderPipeline, RenderPipeline, RenderPipelineCache, RenderPipelineLayout,
        RenderPipelinePlugin,
    },
    shader::Shader,
    texture::{texture_format, GpuTexture},
    CurrentFrame, InitRenderResources, RenderLabel, WgpuDevice,
};

#[derive(Resource)]
pub struct HdrRenderTarget {
    pub texture: GpuTexture,
    pub sampler: Arc<wgpu::Sampler>,
}

impl HdrRenderTarget {
    pub fn color_target(&self) -> &Arc<wgpu::TextureView> {
        &self.texture.view
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.texture = GpuTexture::new(
            device,
            Some("Hdr Render Target"),
            width,
            height,
            texture_format::HDR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
    }
}

impl FromWorld for HdrRenderTarget {
    fn from_world(world: &mut World) -> Self {
        let window_size = world.get_resource::<WindowSize>().unwrap();
        let device = world.get_resource::<WgpuDevice>().unwrap();
        let texture = GpuTexture::new(
            &device,
            Some("Hdr Render Target"),
            window_size.width,
            window_size.height,
            texture_format::HDR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );

        let sampler = Arc::new(device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        }));

        Self { texture, sampler }
    }
}

impl CreateBindGroup for HdrRenderTarget {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Hdr Render Target Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        })
    }

    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        cached_layout: &crate::bind_group::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: cached_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(self.sampler.as_ref()),
                },
            ],
            label: Some("Hdr Render Target Bind Group"),
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HdrNodeLabel;
impl RenderLabel for HdrNodeLabel {}

#[derive(Default)]
pub struct HdrNode;

impl CreateRenderPipeline for HdrNode {
    fn create_render_pipeline_layout(
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> RenderPipelineLayout
    where
        Self: Sized,
    {
        let layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("HDR Pipeline Layout"),
                bind_group_layouts: &[
                    &bind_group_layout_cache.get_or_create::<HdrRenderTarget>(device)
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
        let shader = Shader::new(Path::new("assets/shaders/hdr.wgsl"));
        let shader_module = shader.create_shader_module(device);

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HDR Render Pipeline"),
            layout: Some(cached_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture_format::VIEW_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        RenderPipeline::new(pipeline)
    }
}

impl ViewNode for HdrNode {
    type Param = (
        Res<'static, CurrentFrame>,
        Res<'static, RenderPipelineCache>,
        Res<'static, BindGroup<HdrRenderTarget>>,
    );
    type ViewQueryFetch = &'static ViewTarget;
    type ViewQueryFilter = ();

    fn run(
        &self,
        _render_world: &World,
        _graph_ctx: &mut crate::graph::RenderGraphCtx,
        render_ctx: &mut crate::graph::RenderCtx,
        (current_frame, pipeline_cache, bind_group): &SystemParamItem<Self::Param>,
        _view_target: &Ref<ViewTarget>,
    ) -> Result<()> {
        let pipeline = pipeline_cache.get_pipeline_for::<HdrNode>().unwrap();
        let Some(current_frame) = current_frame.inner.as_ref() else {
            return Ok(());
        };
        {
            let mut render_pass =
                render_ctx
                    .command_encoder()
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("HDR Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &current_frame.color_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}

pub struct HdrPlugin;

impl Plugin for HdrPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(RenderPipelinePlugin::<HdrNode>::default())?;
        app.add_plugin(ResourceBindGroupPlugin::<HdrRenderTarget>::default())?;

        app.add_system(init_hdr_render_target, InitRenderResources);

        app.main_app_mut()
            .add_render_main_graph_node::<ViewNodeRunner<HdrNode>>(HdrNodeLabel);

        Ok(())
    }
}

pub fn init_hdr_render_target(world: &mut World) {
    if !world.has_resource::<HdrRenderTarget>() {
        let target = HdrRenderTarget::from_world(world);
        world.insert_resource(target);
    }
}
