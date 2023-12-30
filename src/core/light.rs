use weaver_proc_macro::Component;

use super::color::Color;

pub const MAX_LIGHTS: usize = 16;

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
}

#[derive(Debug, Clone, Copy, Component, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct DirectionalLight {
    pub direction: glam::Vec3,
    _padding: u32,
    pub color: Color,
    pub intensity: f32,
    _padding2: [u32; 3],
}

impl DirectionalLight {
    pub fn new(direction: glam::Vec3, color: Color, intensity: f32) -> Self {
        Self {
            direction: direction.normalize(),
            _padding: 0,
            color,
            intensity,
            _padding2: [0; 3],
        }
    }
}
