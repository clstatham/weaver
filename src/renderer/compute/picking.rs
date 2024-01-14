//! Compute shader version of picking.

use weaver_proc_macro::Resource;
use wgpu::util::DeviceExt;

use crate::{include_shader, renderer::Renderer};

// HACK: this is a temporary solution until i get the Camera trait implemented
use crate::game::camera::FollowCameraController;

#[derive(Debug, Clone, Copy, Default)]
pub struct PickResult {
    pub position: glam::Vec3,
    pub normal: glam::Vec3,
}

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PickResultUniform {
    pub position: glam::Vec4,
    pub normal: glam::Vec4,
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PickerCameraUniform {
    pub inv_view_proj: glam::Mat4,
}

#[derive(Resource)]
#[allow(dead_code)]
pub struct ScreenPicker {
    pub(crate) pipeline: wgpu::ComputePipeline,
    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) result_storage_buffer: wgpu::Buffer,
    pub(crate) result_mapping_buffer: wgpu::Buffer,
    pub(crate) camera_buffer: wgpu::Buffer,
    pub(crate) screen_pos_buffer: wgpu::Buffer,
}

impl ScreenPicker {
    pub fn new(renderer: &Renderer) -> Self {
        let result_storage_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("screen picker buffer"),
                    contents: bytemuck::cast_slice(&[PickResultUniform::default()]),
                    usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::STORAGE,
                });

        let result_mapping_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("screen picker buffer"),
                    contents: bytemuck::cast_slice(&[PickResultUniform::default()]),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                });

        let camera_buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screen picker buffer"),
            size: std::mem::size_of::<PickerCameraUniform>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        let screen_pos_buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screen picker buffer"),
            size: std::mem::size_of::<glam::Vec2>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        let bind_group_layout =
            renderer
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("screen picker bind group layout"),
                    entries: &[
                        // result buffer
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // camera buffer
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // depth texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Depth,
                            },
                            count: None,
                        },
                        // normal texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            },
                            count: None,
                        },
                        // screen pos buffer
                        wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

        let bind_group = renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("screen picker bind group"),
                layout: &bind_group_layout,
                entries: &[
                    // result buffer
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: result_storage_buffer.as_entire_binding(),
                    },
                    // camera buffer
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: camera_buffer.as_entire_binding(),
                    },
                    // depth texture
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&renderer.depth_texture_view),
                    },
                    // normal texture
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&renderer.normal_texture_view),
                    },
                    // screen pos buffer
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: screen_pos_buffer.as_entire_binding(),
                    },
                ],
            });

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("screen picker pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let shader = renderer
            .device
            .create_shader_module(include_shader!("shaders/picking.wgsl"));

        let pipeline = renderer
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("screen picker pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: "main",
            });

        Self {
            pipeline,
            bind_group_layout,
            bind_group,
            result_storage_buffer,
            result_mapping_buffer,
            camera_buffer,
            screen_pos_buffer,
        }
    }

    pub fn pick(
        &self,
        screen_pos: glam::Vec2,
        renderer: &Renderer,
        camera: &FollowCameraController,
    ) -> anyhow::Result<Option<PickResult>> {
        let width = renderer.config.width as f32;
        let height = renderer.config.height as f32;

        if screen_pos.x < 0.0 || screen_pos.x > width || screen_pos.y < 0.0 || screen_pos.y > height
        {
            return Ok(None);
        }

        let encoder = renderer
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let camera_uniform = PickerCameraUniform {
            inv_view_proj: (camera.projection_matrix() * camera.view_matrix()).inverse(),
        };

        renderer.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );

        renderer.queue.write_buffer(
            &self.screen_pos_buffer,
            0,
            bytemuck::cast_slice(&[screen_pos]),
        );

        renderer.queue.submit(std::iter::once(encoder.finish()));

        let mut encoder = renderer
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("screen picker compute pass"),
                timestamp_writes: None,
            });

            // let num_workgroups_x = renderer.config.width / 8;
            // let num_workgroups_y = renderer.config.height / 8;

            let num_workgroups_x = 1;
            let num_workgroups_y = 1;

            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &self.bind_group, &[]);
            cpass.dispatch_workgroups(num_workgroups_x, num_workgroups_y, 1);
        }

        encoder.copy_buffer_to_buffer(
            &self.result_storage_buffer,
            0,
            &self.result_mapping_buffer,
            0,
            std::mem::size_of::<PickResultUniform>() as u64,
        );

        renderer.queue.submit(std::iter::once(encoder.finish()));

        let (tx, rx) = futures_channel::oneshot::channel();
        self.result_mapping_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, |res| tx.send(res).unwrap());
        renderer.device.poll(wgpu::Maintain::Wait);
        pollster::block_on(rx)??;

        let result = {
            let data: &[u8] = &self.result_mapping_buffer.slice(..).get_mapped_range();
            let result: &[PickResultUniform] = bytemuck::cast_slice(data);

            let result = result[0];

            PickResult {
                position: result.position.truncate(),
                normal: result.normal.truncate(),
            }
        };

        self.result_mapping_buffer.unmap();

        Ok(Some(result))
    }
}
