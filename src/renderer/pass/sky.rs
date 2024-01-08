use crate::{
    core::{
        camera::Camera,
        texture::{
            DepthFormat, HdrCubeFormat, HdrD2ArrayFormat, HdrFormat, Skybox, Texture, TextureFormat,
        },
    },
    ecs::{Query, World},
    include_shader,
    renderer::{
        AllocBuffers, BindGroupLayoutCache, LazyBufferHandle, NonFilteringSampler, Renderer,
    },
};

use super::Pass;

pub const SKYBOX_CUBEMAP_SIZE: u32 = 2048;

pub struct SkyRenderPass {
    enabled: bool,
    sampler_bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl SkyRenderPass {
    pub fn new(
        device: &wgpu::Device,
        bind_group_layout_cache: &BindGroupLayoutCache,
        sampler: &wgpu::Sampler,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Skybox Shader"),
            source: wgpu::ShaderSource::Wgsl(include_shader!("sky.wgsl").into()),
        });

        let sampler_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skybox Sampler Bind Group"),
            layout: &bind_group_layout_cache.get_or_create::<NonFilteringSampler>(device),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(sampler),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Skybox Pipeline Layout"),
            bind_group_layouts: &[
                &bind_group_layout_cache.get_or_create::<HdrCubeFormat>(device),
                &bind_group_layout_cache.get_or_create::<NonFilteringSampler>(device),
                &bind_group_layout_cache.get_or_create::<Camera>(device),
            ],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skybox Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: HdrFormat::FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthFormat::FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            enabled: true,
            pipeline,
            sampler_bind_group,
        }
    }
}

impl Pass for SkyRenderPass {
    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn enable(&mut self) {
        self.enabled = true;
    }

    fn disable(&mut self) {
        self.enabled = false;
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_target: &wgpu::TextureView,
        depth_target: &wgpu::TextureView,
        renderer: &Renderer,
        world: &World,
    ) -> anyhow::Result<()> {
        let skybox = Query::<&Skybox>::new(world);
        let skybox = skybox.iter().next().unwrap();
        let skybox_handle = &skybox.texture.alloc_buffers(renderer)?[0];
        let skybox_bind_group = skybox_handle.bind_group().unwrap();

        let camera = Query::<&Camera>::new(world);
        let camera = camera.iter().next().unwrap();
        let camera_handle = &camera.alloc_buffers(renderer)?[0];
        let camera_bind_group = camera_handle.bind_group().unwrap();

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Skybox Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_target,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &skybox_bind_group, &[]);
            render_pass.set_bind_group(1, &self.sampler_bind_group, &[]);
            render_pass.set_bind_group(2, &camera_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}
