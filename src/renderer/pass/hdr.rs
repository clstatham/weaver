use crate::core::texture::Texture;

use super::Pass;

pub struct HdrRenderPass {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    pub(crate) texture: Texture,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
}

impl HdrRenderPass {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let format = wgpu::TextureFormat::Rgba16Float;

        let texture = Texture::create_color_texture(
            device,
            width as usize,
            height as usize,
            Some("HDR Texture"),
            wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            Some(format),
        );

        let layout = Self::bind_group_layout(device);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ],
            label: Some("HDR Texture Bind Group"),
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("hdr.wgsl"));
        let pipeline_layout = Self::pipeline_layout(device);

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
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
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
            pipeline,
            bind_group,
            texture,
            width,
            height,
            format,
        }
    }
}

impl Pass for HdrRenderPass {
    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_texture: &Texture,
        normal_texture: &Texture,
        depth_texture: &Texture,
        world: &weaver_ecs::World,
    ) -> anyhow::Result<()> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("HDR Render Pass Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HDR Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_texture.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HDR Texture Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    fn pipeline_layout(device: &wgpu::Device) -> wgpu::PipelineLayout
    where
        Self: Sized,
    {
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HDR Render Pipeline Layout"),
            bind_group_layouts: &[&Self::bind_group_layout(device)],
            push_constant_ranges: &[],
        })
    }

    fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }
}
