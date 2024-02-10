use fabricate::prelude::*;
use weaver_proc_macro::{BindableComponent, GpuComponent};

use crate::renderer::internals::{LazyBindGroup, LazyGpuHandle};

use super::mesh::MAX_MESHES;

#[derive(Component, Clone, Copy)]
#[script_vtable(
    translation(&Self) -> glam::Vec3,
    rotation(&Self) -> glam::Quat,
    scale(&Self) -> glam::Vec3,
    set_translation(&mut Self, glam::Vec3) -> (),
    set_rotation(&mut Self, glam::Quat) -> (),
    set_scale(&mut Self, glam::Vec3) -> (),
)]
pub struct Transform {
    #[inspect]
    pub translation: glam::Vec3,
    #[inspect]
    pub rotation: glam::Quat,
    #[inspect]
    pub scale: glam::Vec3,
}

impl Transform {
    pub fn new() -> Self {
        Self {
            translation: glam::Vec3::ZERO,
            rotation: glam::Quat::IDENTITY,
            scale: glam::Vec3::ONE,
        }
    }

    pub fn from_translation(translation: glam::Vec3) -> Self {
        Self {
            translation,
            ..Default::default()
        }
    }

    pub fn from_rotation(rotation: glam::Quat) -> Self {
        Self {
            rotation,
            ..Default::default()
        }
    }

    pub fn from_scale(scale: glam::Vec3) -> Self {
        Self {
            scale,
            ..Default::default()
        }
    }

    pub fn from_translation_rotation(translation: glam::Vec3, rotation: glam::Quat) -> Self {
        Self {
            translation,
            rotation,
            ..Default::default()
        }
    }

    pub fn from_translation_scale(translation: glam::Vec3, scale: glam::Vec3) -> Self {
        Self {
            translation,
            scale,
            ..Default::default()
        }
    }

    pub fn from_rotation_scale(rotation: glam::Quat, scale: glam::Vec3) -> Self {
        Self {
            rotation,
            scale,
            ..Default::default()
        }
    }

    pub fn from_translation_rotation_scale(
        translation: glam::Vec3,
        rotation: glam::Quat,
        scale: glam::Vec3,
    ) -> Self {
        Self {
            translation,
            rotation,
            scale,
        }
    }

    pub fn translation(&self) -> glam::Vec3 {
        self.translation
    }

    pub fn rotation(&self) -> glam::Quat {
        self.rotation
    }

    pub fn scale(&self) -> glam::Vec3 {
        self.scale
    }

    pub fn set_translation(&mut self, translation: glam::Vec3) {
        self.translation = translation;
    }

    pub fn set_rotation(&mut self, rotation: glam::Quat) {
        self.rotation = rotation;
    }

    pub fn set_scale(&mut self, scale: glam::Vec3) {
        self.scale = scale;
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Component, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct GlobalTransform {
    pub matrix: glam::Mat4,
}

impl GlobalTransform {
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
    pub fn look_at(&mut self, target: glam::Vec3, up: glam::Vec3) {
        let eye = self.translation();
        self.matrix = glam::Mat4::look_at_rh(eye, target, up).inverse();
    }

    #[inline]
    pub fn translation(&self) -> glam::Vec3 {
        self.matrix.to_scale_rotation_translation().2
    }

    #[inline]
    pub fn rotation(&self) -> glam::Quat {
        self.matrix.to_scale_rotation_translation().1
    }

    #[inline]
    pub fn scale(&self) -> glam::Vec3 {
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

impl Default for GlobalTransform {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, GpuComponent, BindableComponent)]
#[gpu(update = "update")]
pub struct TransformGpuComponent {
    pub matrix: glam::Mat4,

    #[uniform]
    handle: LazyGpuHandle,
    bind_group: LazyBindGroup<Self>,
}

impl TransformGpuComponent {
    pub fn new(matrix: glam::Mat4) -> Self {
        Self {
            matrix,
            handle: LazyGpuHandle::new(
                crate::renderer::internals::GpuResourceType::Uniform {
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    size: std::mem::size_of::<glam::Mat4>(),
                },
                Some("TransformGpuComponent"),
                None,
            ),
            bind_group: LazyBindGroup::default(),
        }
    }

    pub fn update(&self, _world: &World) -> anyhow::Result<()> {
        self.handle.update(&[self.matrix]);
        Ok(())
    }
}

#[derive(Clone, Debug, GpuComponent, BindableComponent)]
#[gpu(update = "update")]
pub struct TransformArray {
    matrices: Vec<glam::Mat4>,

    #[storage]
    handle: LazyGpuHandle,
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

    pub fn push(&mut self, transform: &GlobalTransform) {
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
