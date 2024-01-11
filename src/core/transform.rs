use std::sync::Arc;

use weaver_proc_macro::Component;

use crate::{
    ecs::World,
    renderer::internals::{
        BindGroupLayoutCache, BindableComponent, GpuComponent, GpuResourceManager, LazyBindGroup,
        LazyGpuHandle,
    },
};

use super::mesh::MAX_MESHES;

#[derive(Clone, Copy, Component, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Transform {
    pub matrix: glam::Mat4,
}

impl Transform {
    pub fn new() -> Self {
        Self {
            matrix: glam::Mat4::IDENTITY,
        }
    }

    #[inline]
    pub fn from_scale_rotation_translation(
        scale: glam::Vec3,
        rotation: glam::Quat,
        translation: glam::Vec3,
    ) -> Self {
        Self {
            matrix: glam::Mat4::from_scale_rotation_translation(scale, rotation, translation),
        }
    }

    #[inline]
    pub fn from_translation(translation: glam::Vec3) -> Self {
        Self::from_scale_rotation_translation(glam::Vec3::ONE, glam::Quat::IDENTITY, translation)
    }

    #[inline]
    pub fn from_rotation(rotation: glam::Quat) -> Self {
        Self::from_scale_rotation_translation(glam::Vec3::ONE, rotation, glam::Vec3::ZERO)
    }

    #[inline]
    pub fn from_scale(scale: glam::Vec3) -> Self {
        Self::from_scale_rotation_translation(scale, glam::Quat::IDENTITY, glam::Vec3::ZERO)
    }

    #[inline]
    pub fn translate(&mut self, x: f32, y: f32, z: f32) {
        self.matrix = glam::Mat4::from_translation(glam::Vec3::new(x, y, z)) * self.matrix;
    }

    #[inline]
    pub fn rotate(&mut self, angle: f32, axis: glam::Vec3) {
        // self.matrix *= glam::Mat4::from_axis_angle(axis, angle);
        self.matrix = glam::Mat4::from_axis_angle(axis, angle) * self.matrix;
    }

    #[inline]
    pub fn scale(&mut self, x: f32, y: f32, z: f32) {
        self.matrix = glam::Mat4::from_scale(glam::Vec3::new(x, y, z)) * self.matrix;
    }

    #[inline]
    pub fn look_at(&mut self, target: glam::Vec3, up: glam::Vec3) {
        let eye = self.get_translation();
        self.matrix = glam::Mat4::look_at_rh(eye, target, up).inverse();
    }

    #[inline]
    pub fn get_translation(&self) -> glam::Vec3 {
        self.matrix.to_scale_rotation_translation().2
    }

    #[inline]
    pub fn get_rotation(&self) -> glam::Quat {
        self.matrix.to_scale_rotation_translation().1
    }

    #[inline]
    pub fn get_scale(&self) -> glam::Vec3 {
        self.matrix.to_scale_rotation_translation().0
    }

    #[inline]
    pub fn set_translation(&mut self, translation: glam::Vec3) {
        let (scale, rotation, _) = self.matrix.to_scale_rotation_translation();
        self.matrix = glam::Mat4::from_scale_rotation_translation(scale, rotation, translation);
    }

    #[inline]
    pub fn set_rotation(&mut self, rotation: glam::Quat) {
        let (scale, _, translation) = self.matrix.to_scale_rotation_translation();
        self.matrix = glam::Mat4::from_scale_rotation_translation(scale, rotation, translation);
    }

    #[inline]
    pub fn set_scale(&mut self, scale: glam::Vec3) {
        let (_, rotation, translation) = self.matrix.to_scale_rotation_translation();
        self.matrix = glam::Mat4::from_scale_rotation_translation(scale, rotation, translation);
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Component, Debug)]
pub struct TransformArray {
    matrices: Vec<glam::Mat4>,
    handle: LazyGpuHandle,
    bind_group: LazyBindGroup<Self>,
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct TransformArrayUniform {
    model_matrices: [glam::Mat4; MAX_MESHES], // todo: MAX_TRANSFORMS?
}

impl TransformArray {
    pub fn new() -> Self {
        Self {
            matrices: Vec::new(),
            handle: LazyGpuHandle::new(
                crate::renderer::internals::GpuResourceType::Uniform {
                    usage: wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::COPY_SRC,
                    size: std::mem::size_of::<TransformArrayUniform>(),
                },
                Some("TransformArray"),
                None,
            ),
            bind_group: LazyBindGroup::default(),
        }
    }

    pub fn push(&mut self, transform: &Transform) {
        self.matrices.push(transform.matrix);
    }

    pub fn clear(&mut self) {
        self.matrices.clear();
    }

    pub fn len(&self) -> usize {
        self.matrices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.matrices.is_empty()
    }

    pub fn uniform(&self) -> TransformArrayUniform {
        let mut model_matrices = [glam::Mat4::IDENTITY; MAX_MESHES];
        for (i, matrix) in self.matrices.iter().enumerate() {
            model_matrices[i] = *matrix;
        }
        TransformArrayUniform { model_matrices }
    }
}

impl Default for TransformArray {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuComponent for TransformArray {
    fn lazy_init(
        &self,
        manager: &crate::renderer::internals::GpuResourceManager,
    ) -> anyhow::Result<Vec<crate::renderer::internals::GpuHandle>> {
        Ok(vec![self.handle.lazy_init(manager)?])
    }

    fn update_resources(&self, _world: &World) -> anyhow::Result<()> {
        let uniform = self.uniform();
        self.handle.update(&[uniform]);
        Ok(())
    }

    fn destroy_resources(&self) -> anyhow::Result<()> {
        self.handle.destroy();
        Ok(())
    }
}

impl BindableComponent for TransformArray {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TransformArray Bind Group Layout"),
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

    fn create_bind_group(
        &self,
        manager: &crate::renderer::internals::GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        let handle = self.handle.lazy_init(manager)?;
        let buffer = handle.get_buffer().unwrap();
        let layout = cache.get_or_create::<Self>(manager.device());
        let bind_group = manager
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("TransformArray Bind Group"),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(buffer.as_entire_buffer_binding()),
                }],
            });
        Ok(Arc::new(bind_group))
    }

    fn bind_group(&self) -> Option<Arc<wgpu::BindGroup>> {
        self.bind_group.bind_group()
    }

    fn lazy_init_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        if let Some(bind_group) = self.bind_group.bind_group() {
            return Ok(bind_group);
        }

        let bind_group = self.bind_group.lazy_init_bind_group(manager, cache, self)?;
        Ok(bind_group)
    }
}
