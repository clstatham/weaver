use std::fmt::Debug;

use weaver_proc_macro::{BindableComponent, GpuComponent};

use crate::renderer::internals::{GpuResourceType, LazyBindGroup, LazyGpuHandle};

use fabricate::prelude::*;

use super::color::Color;

pub const MAX_LIGHTS: usize = 32;

#[derive(Clone, Component, GpuComponent, BindableComponent)]
#[gpu(update = "update")]
pub struct PointLight {
    pub position: glam::Vec3,
    pub color: Color,
    pub intensity: f32,
    pub radius: f32,

    #[gpu(handle)]
    #[uniform]
    pub(crate) handle: LazyGpuHandle,
    pub(crate) bind_group: LazyBindGroup<Self>,
}

impl Debug for PointLight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PointLight")
            .field("position", &self.position)
            .field("color", &self.color)
            .field("intensity", &self.intensity)
            .field("radius", &self.radius)
            .finish()
    }
}

impl PointLight {
    pub fn new(position: glam::Vec3, color: Color, intensity: f32, radius: f32) -> Self {
        Self {
            position,
            color,
            intensity,
            radius,

            handle: Self::default_handle(),
            bind_group: LazyBindGroup::default(),
        }
    }

    fn default_handle() -> LazyGpuHandle {
        LazyGpuHandle::new(
            GpuResourceType::Uniform {
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                size: std::mem::size_of::<PointLightUniform>(),
            },
            Some("Point Light"),
            None,
        )
    }

    pub fn view_transform_in_direction(&self, direction: glam::Vec3, up: glam::Vec3) -> glam::Mat4 {
        glam::Mat4::look_at_lh(self.position, self.position + direction, up)
    }

    pub fn projection_transform(&self) -> glam::Mat4 {
        let aspect = 1.0;
        let fov = 90.0f32.to_radians();
        let near = 1.0;
        let far = 100.0;
        glam::Mat4::perspective_lh(fov, aspect, near, far)
    }

    fn update(&self, _world: &World) -> anyhow::Result<()> {
        self.handle.update(&[PointLightUniform::from(self)]);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PointLightUniform {
    pub position: glam::Vec4,
    pub color: glam::Vec4,
    pub projection_transform: glam::Mat4,
    pub intensity: f32,
    pub radius: f32,
    _pad: glam::Vec2,
}

impl From<&PointLight> for PointLightUniform {
    fn from(light: &PointLight) -> Self {
        Self {
            position: glam::Vec4::new(light.position.x, light.position.y, light.position.z, 1.0),
            color: glam::Vec4::new(light.color.r, light.color.g, light.color.b, 1.0),
            projection_transform: light.projection_transform(),
            intensity: light.intensity,
            radius: light.radius,
            _pad: glam::Vec2::ZERO,
        }
    }
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(crate) struct PointLightArrayUniform {
    pub(crate) count: u32,
    _pad: [u32; 3],
    pub(crate) lights: [PointLightUniform; MAX_LIGHTS],
}

#[derive(GpuComponent, BindableComponent)]
#[gpu(update = "update")]
pub(crate) struct PointLightArray {
    pub(crate) lights: Vec<PointLightUniform>,

    #[gpu(handle)]
    #[storage]
    pub(crate) handle: LazyGpuHandle,
    pub(crate) bind_group: LazyBindGroup<Self>,
}

impl PointLightArray {
    pub fn new() -> Self {
        Self {
            lights: Vec::new(),
            handle: Self::default_handle(),
            bind_group: LazyBindGroup::default(),
        }
    }

    #[doc(hidden)]
    fn default_handle() -> LazyGpuHandle {
        LazyGpuHandle::new(
            GpuResourceType::Storage {
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                size: std::mem::size_of::<PointLightArrayUniform>(),
                read_only: true,
            },
            Some("Point Light Array"),
            None,
        )
    }

    pub fn add_light(&mut self, light: &PointLight) {
        self.lights.push(PointLightUniform::from(light));
    }

    pub fn clear(&mut self) {
        self.lights.clear();
    }

    fn update(&self, _world: &World) -> anyhow::Result<()> {
        self.handle.update(&[PointLightArrayUniform {
            count: self.lights.len() as u32,
            _pad: [0; 3],
            lights: {
                let mut lights = [PointLightUniform::default(); MAX_LIGHTS];
                for (i, light) in self.lights.iter().enumerate() {
                    lights[i] = *light;
                }
                lights
            },
        }]);
        Ok(())
    }
}

impl Default for PointLightArray {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
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

    pub fn view_transform(&self) -> glam::Mat4 {
        glam::Mat4::look_at_rh(
            glam::Vec3::ZERO,
            glam::Vec3::new(self.direction.x, self.direction.y, self.direction.z),
            glam::Vec3::Y,
        )
    }

    pub fn projection_transform(&self) -> glam::Mat4 {
        let left = -80.0;
        let right = 80.0;
        let bottom = -80.0;
        let top = 80.0;
        let near = -200.0;
        let far = 300.0;
        glam::Mat4::orthographic_rh(left, right, bottom, top, near, far)
    }
}

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct DirectionalLightUniform {
    pub direction: [f32; 4],
    pub color: [f32; 4],
    pub view_transform: glam::Mat4,
    pub projection_transform: glam::Mat4,
    pub intensity: f32,
    _pad: [f32; 3],
}

impl From<&DirectionalLight> for DirectionalLightUniform {
    fn from(light: &DirectionalLight) -> Self {
        Self {
            direction: [light.direction.x, light.direction.y, light.direction.z, 0.0],
            color: [light.color.r, light.color.g, light.color.b, 1.0],
            view_transform: light.view_transform(),
            projection_transform: light.projection_transform(),
            intensity: light.intensity,
            _pad: [0.0; 3],
        }
    }
}
