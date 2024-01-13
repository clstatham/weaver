use std::sync::Arc;

use weaver_proc_macro::Component;

use crate::{
    ecs::World,
    renderer::internals::{
        BindGroupLayoutCache, BindableComponent, GpuComponent, GpuHandle, GpuResourceManager,
        GpuResourceType, LazyBindGroup, LazyGpuHandle,
    },
};

use super::color::Color;

pub const MAX_LIGHTS: usize = 32;

#[derive(Component)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointLight {
    pub position: glam::Vec3,
    pub color: Color,
    pub intensity: f32,
    pub radius: f32,

    #[cfg_attr(feature = "serde", serde(skip, default = "PointLight::default_handle"))]
    pub(crate) handle: LazyGpuHandle,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub(crate) bind_group: LazyBindGroup<Self>,
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
}

impl GpuComponent for PointLight {
    fn lazy_init(&self, manager: &GpuResourceManager) -> anyhow::Result<Vec<GpuHandle>> {
        Ok(vec![self.handle.lazy_init(manager)?])
    }

    fn update_resources(&self, _world: &World) -> anyhow::Result<()> {
        self.handle.update(&[PointLightUniform::from(self)]);
        Ok(())
    }

    fn destroy_resources(&self) -> anyhow::Result<()> {
        self.handle.mark_destroyed();
        Ok(())
    }
}

impl BindableComponent for PointLight {
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

    fn create_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        let layout = cache.get_or_create::<Self>(manager.device());
        let buffer = self.handle.lazy_init(manager)?;

        let bind_group = manager
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Point Light Bind Group"),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.get_buffer().unwrap().as_entire_binding(),
                }],
            });
        Ok(Arc::new(bind_group))
    }

    fn bind_group(&self) -> Option<Arc<wgpu::BindGroup>> {
        self.bind_group.bind_group().clone()
    }

    fn lazy_init_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &crate::renderer::internals::BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        if let Some(bind_group) = self.bind_group.bind_group() {
            return Ok(bind_group);
        }

        let bind_group = self.bind_group.lazy_init_bind_group(manager, cache, self)?;
        Ok(bind_group)
    }
}

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable, Component)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct PointLightUniform {
    pub position: [f32; 4],
    pub color: [f32; 4],
    pub projection_transform: glam::Mat4,
    pub intensity: f32,
    pub radius: f32,
    #[cfg_attr(feature = "serde", serde(skip))]
    _pad: [f32; 2],
}

impl From<&PointLight> for PointLightUniform {
    fn from(light: &PointLight) -> Self {
        Self {
            position: [light.position.x, light.position.y, light.position.z, 1.0],
            color: [light.color.r, light.color.g, light.color.b, 1.0],
            projection_transform: light.projection_transform(),
            intensity: light.intensity,
            radius: light.radius,
            _pad: [0.0; 2],
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

#[derive(Component)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct PointLightArray {
    pub(crate) lights: Vec<PointLightUniform>,
    #[cfg_attr(
        feature = "serde",
        serde(skip, default = "PointLightArray::default_handle")
    )]
    pub(crate) handle: LazyGpuHandle,
    #[cfg_attr(feature = "serde", serde(skip))]
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
}

impl Default for PointLightArray {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuComponent for PointLightArray {
    fn lazy_init(&self, manager: &GpuResourceManager) -> anyhow::Result<Vec<GpuHandle>> {
        Ok(vec![self.handle.lazy_init(manager)?])
    }

    fn update_resources(&self, _world: &World) -> anyhow::Result<()> {
        let mut uniform = PointLightArrayUniform {
            count: self.lights.len() as u32,
            _pad: [0; 3],
            lights: [PointLightUniform::default(); MAX_LIGHTS],
        };
        for (i, light) in self.lights.iter().enumerate() {
            uniform.lights[i] = *light;
        }
        self.handle.update(&[uniform]);
        Ok(())
    }

    fn destroy_resources(&self) -> anyhow::Result<()> {
        self.handle.mark_destroyed();
        Ok(())
    }
}

impl BindableComponent for PointLightArray {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Point Light Array Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    fn create_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        let layout = cache.get_or_create::<Self>(manager.device());
        let buffer = self.handle.lazy_init(manager)?;

        let bind_group = manager
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Point Light Array Bind Group"),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.get_buffer().unwrap().as_entire_binding(),
                }],
            });
        Ok(Arc::new(bind_group))
    }

    fn bind_group(&self) -> Option<Arc<wgpu::BindGroup>> {
        self.bind_group.bind_group().clone()
    }

    fn lazy_init_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &crate::renderer::internals::BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        if let Some(bind_group) = self.bind_group.bind_group() {
            return Ok(bind_group);
        }

        let bind_group = self.bind_group.lazy_init_bind_group(manager, cache, self)?;
        Ok(bind_group)
    }
}

#[derive(Debug, Clone, Copy, Component)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
