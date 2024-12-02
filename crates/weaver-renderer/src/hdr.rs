use std::{path::Path, sync::Arc};

use weaver_app::{plugin::Plugin, App};
use weaver_ecs::{
    component::Res,
    prelude::{ResMut, World},
    world::ConstructFromWorld,
};
use weaver_util::prelude::*;
use weaver_winit::WindowSize;

use crate::{
    bind_group::{BindGroup, BindGroupLayoutCache, CreateBindGroup, ResourceBindGroupPlugin},
    pipeline::{
        CreateRenderPipeline, RenderPipeline, RenderPipelineCache, RenderPipelineLayout,
        RenderPipelinePlugin,
    },
    resources::ActiveCommandEncoder,
    shader::Shader,
    texture::{texture_format, GpuTexture},
    CurrentFrame, RenderStage, WgpuDevice,
};

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

impl ConstructFromWorld for HdrRenderTarget {
    fn from_world(world: &World) -> Self {
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

#[derive(Default)]
pub struct HdrRenderable;

impl CreateRenderPipeline for HdrRenderable {
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
            cache: None,
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture_format::VIEW_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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

pub async fn render_hdr(
    mut encoder: ResMut<ActiveCommandEncoder>,
    current_frame: Res<CurrentFrame>,
    pipeline_cache: Res<RenderPipelineCache>,
    bind_group: Res<BindGroup<HdrRenderTarget>>,
) {
    let pipeline = pipeline_cache.get_pipeline_for::<HdrRenderable>().unwrap();
    let Some(current_frame) = current_frame.inner.as_ref() else {
        return;
    };
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

pub struct HdrPlugin;

impl Plugin for HdrPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(RenderPipelinePlugin::<HdrRenderable>::default())?;
        app.add_plugin(ResourceBindGroupPlugin::<HdrRenderTarget>::default())?;

        // app.main_app_mut().add_renderable::<HdrRenderable>();

        app.main_app_mut()
            .world_mut()
            .add_system(render_hdr, RenderStage::Render);

        Ok(())
    }

    fn ready(&self, app: &App) -> bool {
        app.has_resource::<WgpuDevice>()
    }

    fn finish(&self, app: &mut App) -> Result<()> {
        app.init_resource::<HdrRenderTarget>();
        Ok(())
    }
}
