use weaver_ecs::{Query, Queryable, Read};

use crate::{
    core::{camera::Camera, light::PointLight, texture::Texture},
    include_shader,
};

use super::Pass;

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct LightUniform {
    pub position: glam::Vec3,
    _padding: u32,
    pub color: glam::Vec3,
    _padding2: u32,
    pub intensity: f32,
    _padding3: [u32; 3],
}

pub struct PhongRenderPass {
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) light_buffer: wgpu::Buffer,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) color_texture_copy: Texture,
    pub(crate) depth_texture_copy: Texture,
    pub(crate) normal_texture_copy: Texture,
    pub(crate) camera_pos_buffer: wgpu::Buffer,
    pub(crate) inverse_camera_proj_buffer: wgpu::Buffer,
}

impl PhongRenderPass {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> anyhow::Result<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Phong Shader"),
            source: wgpu::ShaderSource::Wgsl(include_shader!("phong.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Phong Render Pipeline"),
            layout: Some(&Self::pipeline_layout(device)),
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

        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Buffer"),
            size: std::mem::size_of::<LightUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let inverse_camera_proj_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Inverse Camera Projection Buffer"),
            size: std::mem::size_of::<glam::Mat4>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let color_texture_copy = Texture::create_color_texture(
            device,
            config.width as usize,
            config.height as usize,
            Some("Color Texture Copy"),
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        );

        let depth_texture_copy = Texture::create_depth_texture(
            device,
            config.width as usize,
            config.height as usize,
            Some("Depth Texture Copy"),
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        );

        let normal_texture_copy = Texture::create_normal_texture(
            device,
            config.width as usize,
            config.height as usize,
            Some("Normal Texture Copy"),
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        );

        let camera_pos_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Position Buffer"),
            size: std::mem::size_of::<glam::Vec3>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Light Bind Group"),
            layout: &Self::bind_group_layout(device),
            entries: &[
                // color texture
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&color_texture_copy.view),
                },
                // color texture sampler
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&color_texture_copy.sampler),
                },
                // normal texture
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&normal_texture_copy.view),
                },
                // normal texture sampler
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&normal_texture_copy.sampler),
                },
                // depth texture
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&depth_texture_copy.view),
                },
                // depth texture sampler
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&depth_texture_copy.sampler),
                },
                // light buffer
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Buffer(
                        light_buffer.as_entire_buffer_binding(),
                    ),
                },
                // inverse camera projection matrix
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Buffer(
                        inverse_camera_proj_buffer.as_entire_buffer_binding(),
                    ),
                },
                // camera position
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Buffer(
                        camera_pos_buffer.as_entire_buffer_binding(),
                    ),
                },
            ],
        });

        Ok(Self {
            pipeline,
            light_buffer,
            bind_group,
            inverse_camera_proj_buffer,
            color_texture_copy,
            depth_texture_copy,
            normal_texture_copy,
            camera_pos_buffer,
        })
    }
}
impl Pass for PhongRenderPass {
    fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Phong Bind Group Layout"),
            entries: &[
                // color texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // color texture sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // normal texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // normal texture sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // depth texture
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // depth texture sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // light buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // inverse camera projection matrix
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // camera position
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
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
            label: Some("Phong Pipeline Layout"),
            bind_group_layouts: &[&Self::bind_group_layout(device)],
            push_constant_ranges: &[],
        })
    }

    fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

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
            label: Some("Phong Render Pass Initial Encoder"),
        });

        let camera = world.read_resource::<Camera>();
        queue.write_buffer(
            &self.inverse_camera_proj_buffer,
            0,
            bytemuck::cast_slice(&[camera.projection_matrix().inverse()]),
        );
        queue.write_buffer(
            &self.camera_pos_buffer,
            0,
            bytemuck::cast_slice(&[camera.eye]),
        );

        encoder.copy_texture_to_texture(
            color_texture.texture.as_image_copy(),
            self.color_texture_copy.texture.as_image_copy(),
            wgpu::Extent3d {
                width: self.color_texture_copy.texture.width(),
                height: self.color_texture_copy.texture.height(),
                depth_or_array_layers: 1,
            },
        );

        encoder.copy_texture_to_texture(
            depth_texture.texture.as_image_copy(),
            self.depth_texture_copy.texture.as_image_copy(),
            wgpu::Extent3d {
                width: self.depth_texture_copy.texture.width(),
                height: self.depth_texture_copy.texture.height(),
                depth_or_array_layers: 1,
            },
        );

        encoder.copy_texture_to_texture(
            normal_texture.texture.as_image_copy(),
            self.normal_texture_copy.texture.as_image_copy(),
            wgpu::Extent3d {
                width: self.normal_texture_copy.texture.width(),
                height: self.normal_texture_copy.texture.height(),
                depth_or_array_layers: 1,
            },
        );

        queue.submit(std::iter::once(encoder.finish()));

        let query = world.query::<Query<Read<PointLight>>>();

        for light in query.iter() {
            let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Phong Render Pass Buffer Write Encoder"),
            });

            let light_uniform = LightUniform {
                position: light.position,
                _padding: 0,
                color: light.color.vec3(),
                _padding2: 0,
                intensity: light.intensity,
                _padding3: [0; 3],
            };

            queue.write_buffer(
                &self.light_buffer,
                0,
                bytemuck::cast_slice(&[light_uniform]),
            );

            queue.submit(std::iter::once(encoder.finish()));

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Phong Render Pass Encoder"),
            });
            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Phong Render Pass"),
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

                render_pass.set_pipeline(self.pipeline());
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.draw(0..6, 0..1);
            }

            queue.submit(std::iter::once(encoder.finish()));
        }

        Ok(())
    }
}
