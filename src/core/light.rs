use weaver_proc_macro::Component;

use crate::renderer::{
    AllocBuffers, BufferHandle, CreateBindGroupLayout, LazyBufferHandle, Renderer,
};

use super::color::Color;

pub const MAX_LIGHTS: usize = 64;

#[derive(Clone, Component)]
pub struct PointLight {
    pub position: glam::Vec3,
    pub color: Color,
    pub intensity: f32,

    pub(crate) handle: LazyBufferHandle,
}

impl PointLight {
    pub fn new(position: glam::Vec3, color: Color, intensity: f32) -> Self {
        Self {
            position,
            color,
            intensity,

            handle: LazyBufferHandle::new(
                crate::renderer::BufferBindingType::Uniform {
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    size: Some(std::mem::size_of::<PointLightUniform>()),
                },
                Some("Point Light"),
                None,
            ),
        }
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
}

impl AllocBuffers for PointLight {
    fn alloc_buffers(&self, renderer: &Renderer) -> anyhow::Result<Vec<BufferHandle>> {
        Ok(vec![self.handle.get_or_create_init::<_, Self>(
            renderer,
            &[PointLightUniform::from(self)],
        )])
    }
}

impl CreateBindGroupLayout for PointLight {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
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

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PointLightUniform {
    pub position: [f32; 4],
    pub color: [f32; 4],
    pub projection_transform: glam::Mat4,
    pub intensity: f32,
    _pad: [f32; 3],
}

impl From<&PointLight> for PointLightUniform {
    fn from(light: &PointLight) -> Self {
        Self {
            position: [light.position.x, light.position.y, light.position.z, 1.0],
            color: [light.color.r, light.color.g, light.color.b, 1.0],
            projection_transform: light.projection_transform(),
            intensity: light.intensity,
            _pad: [0.0; 3],
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

#[derive(Clone, Component)]
pub(crate) struct PointLightArray {
    pub(crate) lights: Vec<PointLightUniform>,
    pub(crate) handle: LazyBufferHandle,
}

impl PointLightArray {
    pub fn new() -> Self {
        Self {
            lights: Vec::new(),
            handle: LazyBufferHandle::new(
                crate::renderer::BufferBindingType::Storage {
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    size: Some(std::mem::size_of::<PointLightArrayUniform>()),
                    read_only: true,
                },
                Some("Point Light Array"),
                None,
            ),
        }
    }

    pub fn add_light(&mut self, light: &PointLight) {
        self.lights.push(PointLightUniform::from(light));
    }

    pub fn clear(&mut self) {
        self.lights.clear();
    }

    pub fn update(&mut self) {
        let mut uniform = PointLightArrayUniform {
            count: self.lights.len() as u32,
            _pad: [0; 3],
            lights: [PointLightUniform::default(); MAX_LIGHTS],
        };
        for (i, light) in self.lights.iter().enumerate() {
            uniform.lights[i] = *light;
        }
        self.handle.update::<PointLightArrayUniform>(&[uniform]);
    }
}

impl Default for PointLightArray {
    fn default() -> Self {
        Self::new()
    }
}

impl CreateBindGroupLayout for PointLightArray {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Point Light Array Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }
}

impl AllocBuffers for PointLightArray {
    fn alloc_buffers(&self, renderer: &Renderer) -> anyhow::Result<Vec<BufferHandle>> {
        Ok(vec![self.handle.get_or_create::<Self>(renderer)])
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
