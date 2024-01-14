use crate::{
    core::texture::{HdrTexture, TextureFormat, WindowTexture},
    ecs::World,
    include_shader,
    renderer::{internals::BindableComponent, BindGroupLayoutCache},
};

use super::Pass;

#[allow(dead_code)]
pub struct HdrRenderPass {
    enabled: bool,

    pipeline_layout: wgpu::PipelineLayout,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    pub(crate) texture: HdrTexture,
}

impl HdrRenderPass {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        bind_group_layout_cache: &BindGroupLayoutCache,
    ) -> Self {
        let texture = HdrTexture::new(width, height, Some("HDR Texture"));

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HDR Texture Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let sampler_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("HDR Texture Sampler Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                }],
            });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &sampler_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&sampler),
            }],
            label: Some("HDR Texture Bind Group"),
        });

        let shader = device.create_shader_module(include_shader!("hdr.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HDR Render Pipeline Layout"),
            bind_group_layouts: &[
                &sampler_bind_group_layout,
                &bind_group_layout_cache.get_or_create::<HdrTexture>(device),
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
                    format: WindowTexture::FORMAT,
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
            sampler,
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
        let texture_bind_group = self.texture.lazy_init_bind_group(
            &renderer.resource_manager,
            &renderer.bind_group_layout_cache,
        )?;

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
