use crate::{
    core::texture::{HdrFormat, Texture, TextureFormat, WindowFormat},
    ecs::World,
    renderer::{AllocBuffers, BindGroupLayoutCache, NonFilteringSampler},
};

use super::Pass;

#[allow(dead_code)]
pub struct HdrRenderPass {
    enabled: bool,

    pipeline_layout: wgpu::PipelineLayout,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    pub(crate) texture: Texture<HdrFormat>,
}

impl HdrRenderPass {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        sampler: &wgpu::Sampler,
        bind_group_layout_cache: &BindGroupLayoutCache,
    ) -> Self {
        let format = HdrFormat::FORMAT;

        let texture = Texture::new_lazy(
            width,
            height,
            Some("HDR Texture"),
            wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            wgpu::TextureDimension::D2,
            wgpu::TextureViewDimension::D2,
            1,
        );

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout_cache.get_or_create::<NonFilteringSampler>(device),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(sampler),
            }],
            label: Some("HDR Texture Bind Group"),
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("hdr.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HDR Render Pipeline Layout"),
            bind_group_layouts: &[
                &bind_group_layout_cache.get_or_create::<NonFilteringSampler>(device),
                &bind_group_layout_cache.get_or_create::<Texture<HdrFormat>>(device),
            ],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HDR Render Pipeline"),
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
                    format: WindowFormat::FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
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

        Self {
            enabled: true,
            pipeline,
            bind_group,
            texture,
            pipeline_layout,
        }
    }
}

impl Pass for HdrRenderPass {
    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn enable(&mut self) {
        self.enabled = true;
    }

    fn disable(&mut self) {
        self.enabled = false;
    }

    fn prepare(&self, _world: &World, _renderer: &crate::renderer::Renderer) -> anyhow::Result<()> {
        Ok(())
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_target: &wgpu::TextureView,
        _depth_target: &wgpu::TextureView,
        renderer: &crate::renderer::Renderer,
        _world: &World,
    ) -> anyhow::Result<()> {
        let texture_handle = &self.texture.alloc_buffers(renderer)?[0];
        let texture_bind_group = texture_handle.bind_group().unwrap();

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HDR Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_bind_group(1, &texture_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}
