use wgpu::util::DeviceExt;

use crate::ecs::component::Component;

use super::Vertex;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Transform {
    pub matrix: glam::Mat4,
}
impl Component for Transform {}

impl Transform {
    pub fn new() -> Self {
        Self {
            matrix: glam::Mat4::IDENTITY,
        }
    }

    pub fn translate(&mut self, x: f32, y: f32, z: f32) -> Self {
        self.matrix *= glam::Mat4::from_translation(glam::Vec3::new(x, y, z));
        *self
    }

    pub fn rotate(&mut self, angle: f32, axis: glam::Vec3) -> Self {
        self.matrix *= glam::Mat4::from_axis_angle(axis, angle);
        *self
    }

    pub fn scale(&mut self, x: f32, y: f32, z: f32) -> Self {
        self.matrix *= glam::Mat4::from_scale(glam::Vec3::new(x, y, z));
        *self
    }

    pub fn transform_vertex(&self, vertex: Vertex) -> Vertex {
        let position = self.matrix.transform_point3(vertex.position);
        let normal = self.matrix.transform_vector3(vertex.normal).normalize();
        let color = vertex.color;
        let uv = vertex.uv;

        Vertex {
            position,
            normal,
            color,
            uv,
        }
    }

    pub fn create_bind_group_and_buffer(
        &self,
        device: &wgpu::Device,
    ) -> (wgpu::BindGroup, wgpu::Buffer) {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Transform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Transform Uniform Buffer"),
            contents: bytemuck::cast_slice(&[self.matrix]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Transform Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(uniform_buffer.as_entire_buffer_binding()),
            }],
        });

        (bind_group, uniform_buffer)
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}
