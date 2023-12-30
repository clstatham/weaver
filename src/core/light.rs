use weaver_proc_macro::Component;

use super::color::Color;

pub const MAX_LIGHTS: usize = 16;

#[derive(Debug, Clone, Copy, Component)]
pub struct PointLight {
    pub position: glam::Vec3,
    pub color: Color,
    pub intensity: f32,
}

impl PointLight {
    pub fn new(position: glam::Vec3, color: Color, intensity: f32) -> Self {
        Self {
            position,
            color,
            intensity,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PointLightUniform {
    pub position: [f32; 4],
    pub color: [f32; 4],
    pub intensity: f32,
    _pad: [f32; 3],
}

impl From<&PointLight> for PointLightUniform {
    fn from(light: &PointLight) -> Self {
        Self {
            position: [light.position.x, light.position.y, light.position.z, 1.0],
            color: [light.color.r, light.color.g, light.color.b, 1.0],
            intensity: light.intensity,
            _pad: [0.0; 3],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PointLightBuffer {
    pub lights: [PointLightUniform; MAX_LIGHTS],
    pub count: u32,
    _pad: [u32; 3],
}

impl PointLightBuffer {
    pub fn push(&mut self, light: PointLightUniform) {
        self.lights[self.count as usize] = light;
        self.count += 1;
    }

    pub fn clear(&mut self) {
        self.count = 0;
    }
}

impl From<&[PointLight]> for PointLightBuffer {
    fn from(lights: &[PointLight]) -> Self {
        let mut buffer = Self::default();
        for light in lights {
            buffer.push(light.into());
        }
        buffer
    }
}

#[derive(Debug, Clone, Copy, Component)]
pub struct DirectionalLight {
    pub direction: glam::Vec3,
    pub color: Color,
    pub intensity: f32,
}

impl DirectionalLight {
    pub fn new(direction: glam::Vec3, color: Color, intensity: f32) -> Self {
        Self {
            direction,
            color,
            intensity,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct DirectionalLightUniform {
    pub direction: [f32; 4],
    pub color: [f32; 4],
    pub intensity: f32,
    _pad: [f32; 3],
}

impl From<&DirectionalLight> for DirectionalLightUniform {
    fn from(light: &DirectionalLight) -> Self {
        Self {
            direction: [light.direction.x, light.direction.y, light.direction.z, 0.0],
            color: [light.color.r, light.color.g, light.color.b, 1.0],
            intensity: light.intensity,
            _pad: [0.0; 3],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct DirectionalLightBuffer {
    pub lights: [DirectionalLightUniform; MAX_LIGHTS],
    pub count: u32,
    _pad: [u32; 3],
}

impl DirectionalLightBuffer {
    pub fn push(&mut self, light: DirectionalLightUniform) {
        self.lights[self.count as usize] = light;
        self.count += 1;
    }

    pub fn clear(&mut self) {
        self.count = 0;
    }
}

impl From<&[DirectionalLight]> for DirectionalLightBuffer {
    fn from(lights: &[DirectionalLight]) -> Self {
        let mut buffer = Self::default();
        for light in lights {
            buffer.push(light.into());
        }
        buffer
    }
}
