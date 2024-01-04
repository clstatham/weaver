use weaver_proc_macro::Component;

#[derive(Debug, Clone, Copy, PartialEq, Component, bytemuck::Pod, bytemuck::Zeroable)]
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
    pub fn translate(&mut self, x: f32, y: f32, z: f32) -> Self {
        self.matrix = glam::Mat4::from_translation(glam::Vec3::new(x, y, z)) * self.matrix;
        *self
    }

    #[inline]
    pub fn rotate(&mut self, angle: f32, axis: glam::Vec3) -> Self {
        // self.matrix *= glam::Mat4::from_axis_angle(axis, angle);
        self.matrix = glam::Mat4::from_axis_angle(axis, angle) * self.matrix;
        *self
    }

    #[inline]
    pub fn scale(&mut self, x: f32, y: f32, z: f32) -> Self {
        self.matrix = glam::Mat4::from_scale(glam::Vec3::new(x, y, z)) * self.matrix;
        *self
    }

    #[inline]
    pub fn looking_at(&mut self, target: glam::Vec3, up: glam::Vec3) -> Self {
        let eye = self.get_translation();
        self.matrix = glam::Mat4::look_at_rh(eye, target, up).inverse();
        *self
    }

    #[inline]
    pub fn get_translation(self) -> glam::Vec3 {
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
    pub fn set_translation(&mut self, translation: glam::Vec3) -> Self {
        let (scale, rotation, _) = self.matrix.to_scale_rotation_translation();
        self.matrix = glam::Mat4::from_scale_rotation_translation(scale, rotation, translation);
        *self
    }

    #[inline]
    pub fn set_rotation(&mut self, rotation: glam::Quat) -> Self {
        let (scale, _, translation) = self.matrix.to_scale_rotation_translation();
        self.matrix = glam::Mat4::from_scale_rotation_translation(scale, rotation, translation);
        *self
    }

    #[inline]
    pub fn set_scale(&mut self, scale: glam::Vec3) -> Self {
        let (_, rotation, translation) = self.matrix.to_scale_rotation_translation();
        self.matrix = glam::Mat4::from_scale_rotation_translation(scale, rotation, translation);
        *self
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}
