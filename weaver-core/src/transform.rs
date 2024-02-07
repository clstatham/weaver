use fabricate::prelude::*;
use weaver_proc_macro::{BindableComponent, GpuComponent};

use crate::renderer::internals::{LazyBindGroup, LazyGpuHandle};

use super::mesh::MAX_MESHES;

// #[derive(Atom, Clone, Copy)]
// #[script_vtable(
//     translation(Self) -> glam::Vec3,
//     rotation(Self) -> glam::Quat,
//     scale(Self) -> glam::Vec3,
//     set_translation(&mut Self, &glam::Vec3) -> (),
//     set_rotation(&mut Self, &glam::Quat) -> (),
//     set_scale(&mut Self, &glam::Vec3) -> ()
// )]
#[derive(Clone, Copy)]
pub struct Transform {
    pub translation: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
}

impl fabricate::component::Atom for Transform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_any_box(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn fabricate::component::Atom> {
        Box::new(self.clone())
    }
    fn script_vtable(&self) -> fabricate::component::ScriptVtable {
        let mut map = std::collections::HashMap::default();
        map.insert(
            stringify!(translation).to_string(),
            fabricate::component::ScriptMethod {
                name: stringify!(translation).to_string(),
                args: vec![<Self as fabricate::registry::StaticId>::static_type_uid()],
                ret: <glam::Vec3 as fabricate::registry::StaticId>::static_type_uid(),
                takes_self: fabricate::component::TakesSelf::Ref,
                run: |mut args| {
                    let [arg0] = &mut args[..] else {
                        fabricate::prelude::bail!("Wrong number of args")
                    };
                    let arg0 = arg0.as_ref::<Self>().unwrap();
                    let ret = Self::translation(arg0);
                    Ok(vec![fabricate::storage::Data::new_dynamic(ret)])
                },
            },
        );
        map.insert(
            stringify!(rotation).to_string(),
            fabricate::component::ScriptMethod {
                name: stringify!(rotation).to_string(),
                args: vec![<Self as fabricate::registry::StaticId>::static_type_uid()],
                ret: <glam::Quat as fabricate::registry::StaticId>::static_type_uid(),
                takes_self: fabricate::component::TakesSelf::Ref,
                run: |mut args| {
                    let [arg0] = &mut args[..] else {
                        fabricate::prelude::bail!("Wrong number of args")
                    };
                    let arg0 = arg0.as_ref::<Self>().unwrap();
                    let ret = Self::rotation(arg0);
                    Ok(vec![fabricate::storage::Data::new_dynamic(ret)])
                },
            },
        );
        map.insert(
            stringify!(scale).to_string(),
            fabricate::component::ScriptMethod {
                name: stringify!(scale).to_string(),
                args: vec![<Self as fabricate::registry::StaticId>::static_type_uid()],
                ret: <glam::Vec3 as fabricate::registry::StaticId>::static_type_uid(),
                takes_self: fabricate::component::TakesSelf::Ref,
                run: |mut args| {
                    let [arg0] = &mut args[..] else {
                        fabricate::prelude::bail!("Wrong number of args")
                    };
                    let arg0 = arg0.as_ref::<Self>().unwrap();
                    let ret = Self::scale(arg0);
                    Ok(vec![fabricate::storage::Data::new_dynamic(ret)])
                },
            },
        );
        map.insert(
            stringify!(set_translation).to_string(),
            fabricate::component::ScriptMethod {
                name: stringify!(set_translation).to_string(),
                args: vec![
                    <&mut Self as fabricate::registry::StaticId>::static_type_uid(),
                    <&glam::Vec3 as fabricate::registry::StaticId>::static_type_uid(),
                ],
                ret: <() as fabricate::registry::StaticId>::static_type_uid(),
                takes_self: fabricate::component::TakesSelf::RefMut,
                run: |mut args| {
                    let [arg0, arg1] = &mut args[..] else {
                        fabricate::prelude::bail!("Wrong number of args")
                    };
                    let mut arg0 = arg0.as_mut::<Self>().unwrap();
                    let arg1 = arg1.as_ref::<glam::Vec3>().unwrap();
                    let ret = Self::set_translation(arg0, arg1);
                    Ok(vec![fabricate::storage::Data::new_dynamic(ret)])
                },
            },
        );
        map.insert(
            stringify!(set_rotation).to_string(),
            fabricate::component::ScriptMethod {
                name: stringify!(set_rotation).to_string(),
                args: vec![
                    <&mut Self as fabricate::registry::StaticId>::static_type_uid(),
                    <&glam::Quat as fabricate::registry::StaticId>::static_type_uid(),
                ],
                ret: <() as fabricate::registry::StaticId>::static_type_uid(),
                takes_self: fabricate::component::TakesSelf::RefMut,
                run: |mut args| {
                    let [arg0, arg1] = &mut args[..] else {
                        fabricate::prelude::bail!("Wrong number of args")
                    };
                    let mut arg0 = arg0.as_mut::<Self>().unwrap();
                    let arg1 = arg1.as_ref::<glam::Quat>().unwrap();
                    let ret = Self::set_rotation(arg0, arg1);
                    Ok(vec![fabricate::storage::Data::new_dynamic(ret)])
                },
            },
        );
        map.insert(
            stringify!(set_scale).to_string(),
            fabricate::component::ScriptMethod {
                name: stringify!(set_scale).to_string(),
                args: vec![
                    <&mut Self as fabricate::registry::StaticId>::static_type_uid(),
                    <&glam::Vec3 as fabricate::registry::StaticId>::static_type_uid(),
                ],
                ret: <() as fabricate::registry::StaticId>::static_type_uid(),
                takes_self: fabricate::component::TakesSelf::RefMut,
                run: |mut args| {
                    let [arg0, arg1] = &mut args[..] else {
                        fabricate::prelude::bail!("Wrong number of args")
                    };
                    let mut arg0 = arg0.as_mut::<Self>().unwrap();
                    let arg1 = arg1.as_ref::<glam::Vec3>().unwrap();
                    let ret = Self::set_scale(arg0, arg1);
                    Ok(vec![fabricate::storage::Data::new_dynamic(ret)])
                },
            },
        );
        fabricate::component::ScriptVtable { methods: map }
    }
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

    pub fn set_translation(&mut self, translation: &glam::Vec3) {
        self.translation = *translation;
    }

    pub fn set_rotation(&mut self, rotation: &glam::Quat) {
        self.rotation = *rotation;
    }

    pub fn set_scale(&mut self, scale: &glam::Vec3) {
        self.scale = *scale;
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Atom, bytemuck::Pod, bytemuck::Zeroable)]
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
