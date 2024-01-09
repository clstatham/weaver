use wgpu::util::DeviceExt;

use crate::{
    core::{
        camera::CameraUniform,
        particles::{ParticleEmitter, TOTAL_MAX_PARTICLES},
        texture::{DepthFormat, TextureFormat, WindowFormat},
    },
    include_shader,
    prelude::*,
    renderer::AllocBuffers,
};

use super::Pass;

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct ParticleUniform {
    position: Vec4,
    color: Vec4,
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct ParticleVertex {
    position: Vec4,
    uv: Vec2,
    _padding: [f32; 2],
}

pub struct ParticleRenderPass {
    enabled: bool,

    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    // particle storage buffer
    particle_buffer: wgpu::Buffer,
    // particle quad vertex buffer
    particle_quad_buffer: wgpu::Buffer,
    // camera uniform buffer
    camera_buffer: wgpu::Buffer,
    // sampler
    sampler: wgpu::Sampler,
    // sampler bind group
    sampler_bind_group: wgpu::BindGroup,
}

impl ParticleRenderPass {
    pub fn new(device: &wgpu::Device) -> Self {
        let particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Buffer"),
            size: std::mem::size_of::<ParticleUniform>() as u64 * TOTAL_MAX_PARTICLES as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let particle_quad_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Quad Buffer"),
            contents: bytemuck::cast_slice(&[
                ParticleVertex {
                    position: Vec4::new(-0.5, -0.5, 0.0, 1.0),
                    uv: Vec2::new(0.0, 0.0),
                    _padding: [0.0; 2],
                },
                ParticleVertex {
                    position: Vec4::new(0.5, -0.5, 0.0, 1.0),
                    uv: Vec2::new(1.0, 0.0),
                    _padding: [0.0; 2],
                },
                ParticleVertex {
                    position: Vec4::new(-0.5, 0.5, 0.0, 1.0),
                    uv: Vec2::new(0.0, 1.0),
                    _padding: [0.0; 2],
                },
                ParticleVertex {
                    position: Vec4::new(0.5, -0.5, 0.0, 1.0),
                    uv: Vec2::new(1.0, 0.0),
                    _padding: [0.0; 2],
                },
                ParticleVertex {
                    position: Vec4::new(0.5, 0.5, 0.0, 1.0),
                    uv: Vec2::new(1.0, 1.0),
                    _padding: [0.0; 2],
                },
                ParticleVertex {
                    position: Vec4::new(-0.5, 0.5, 0.0, 1.0),
                    uv: Vec2::new(0.0, 1.0),
                    _padding: [0.0; 2],
                },
            ]),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Particle Bind Group Layout"),
            entries: &[
                // particle storage buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // camera uniform buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let sampler_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Particle Sampler Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                }],
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Particle Texture Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Particle Pipeline Layout"),
            bind_group_layouts: &[
                &bind_group_layout,
                &texture_bind_group_layout,
                &sampler_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Shader"),
            source: wgpu::ShaderSource::Wgsl(include_shader!("particles.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ParticleVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x2],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: WindowFormat::FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(
                wgpu::DepthStencilState {
                    format: DepthFormat::FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                },
            ),
            multisample: wgpu::MultisampleState::default(),
            multiview: Default::default(),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                // particle storage buffer
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: particle_buffer.as_entire_binding(),
                },
                // camera uniform buffer
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: camera_buffer.as_entire_binding(),
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Particle Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let sampler_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Sampler Bind Group"),
            layout: &sampler_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&sampler),
            }],
        });

        Self {
            enabled: true,
            pipeline,
            bind_group,
            particle_buffer,
            particle_quad_buffer,
            camera_buffer,
            sampler,
            sampler_bind_group,
        }
    }
}

impl Pass for ParticleRenderPass {
    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn enable(&mut self) {
        self.enabled = true;
    }

    fn disable(&mut self) {
        self.enabled = false;
    }

    fn prepare(&self, world: &World, renderer: &Renderer) -> anyhow::Result<()> {
        // todo
        Ok(())
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_target: &wgpu::TextureView,
        depth_target: &wgpu::TextureView,
        renderer: &crate::renderer::Renderer,
        world: &World,
    ) -> anyhow::Result<()> {
        let emitters = Query::<&ParticleEmitter>::new(world);

        todo!("particle render pass");

        Ok(())
    }
}
