use weaver_ecs::prelude::*;
use weaver_proc_macro::{BindableComponent, GpuComponent};

use crate::renderer::internals::{LazyBindGroup, LazyGpuHandle};

use super::mesh::MAX_MESHES;

#[derive(Clone, Copy, Component, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[method(new = "fn() -> Transform")]
#[method(
    from_scale_rotation_translation = "fn(scale: glam::Vec3, rotation: glam::Quat, translation: glam::Vec3) -> Transform"
)]
#[method(from_translation = "fn(translation: glam::Vec3) -> Transform")]
#[method(from_rotation = "fn(rotation: glam::Quat) -> Transform")]
#[method(from_scale = "fn(scale: glam::Vec3) -> Transform")]
#[method(translate = "fn(&mut Transform, x: f32, y: f32, z: f32)")]
#[method(rotate = "fn(&mut Transform, angle: f32, axis: glam::Vec3)")]
#[method(scale = "fn(&mut Transform, x: f32, y: f32, z: f32)")]
#[method(look_at = "fn(&mut Transform, target: glam::Vec3, up: glam::Vec3)")]
#[method(get_translation = "fn(&Transform) -> glam::Vec3")]
#[method(get_rotation = "fn(&Transform) -> glam::Quat")]
#[method(get_scale = "fn(&Transform) -> glam::Vec3")]
#[method(set_translation = "fn(&mut Transform, translation: glam::Vec3)")]
#[method(set_rotation = "fn(&mut Transform, rotation: glam::Quat)")]
#[method(set_scale = "fn(&mut Transform, scale: glam::Vec3)")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

#[derive(Clone, Component, Debug, GpuComponent, BindableComponent)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[gpu(update = "update")]
pub struct TransformArray {
    matrices: Vec<glam::Mat4>,
    #[cfg_attr(
        feature = "serde",
        serde(skip, default = "TransformArray::default_handle")
    )]
    #[storage]
    handle: LazyGpuHandle,
    #[cfg_attr(feature = "serde", serde(skip))]
    bind_group: LazyBindGroup<Self>,
}

impl TransformArray {
    pub fn new() -> Self {
        Self {
            matrices: Vec::new(),
            handle: Self::default_handle(),
            bind_group: LazyBindGroup::default(),
        }
    }

    fn default_handle() -> LazyGpuHandle {
        LazyGpuHandle::new(
            crate::renderer::internals::GpuResourceType::Storage {
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                size: std::mem::size_of::<glam::Mat4>() * MAX_MESHES,
                read_only: true,
            },
            Some("TransformArray"),
            None,
        )
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

    pub fn update(&self, _world: &World) -> anyhow::Result<()> {
        self.handle.update(&self.matrices);
        Ok(())
    }
}

impl Default for TransformArray {
    fn default() -> Self {
        Self::new()
    }
}
