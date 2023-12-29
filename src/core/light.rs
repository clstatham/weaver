use weaver_proc_macro::Component;

use super::color::Color;

pub const MAX_LIGHTS: usize = 16;

#[derive(Debug, Clone, Copy, Component)]
pub enum Light {
    Point(PointLight),
    // todo: Directional
    // todo: Spot
}

#[derive(Debug, Clone, Copy, Component, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PointLight {
    pub position: glam::Vec3,
    _padding: u32,
    pub color: Color,
    pub intensity: f32,
    _padding2: [u32; 3],
}

impl PointLight {
    pub fn new(position: glam::Vec3, color: Color, intensity: f32) -> Self {
        Self {
            position,
            _padding: 0,
            color,
            intensity,
            _padding2: [0; 3],
        }
    }

    pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Point Light Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }
}
