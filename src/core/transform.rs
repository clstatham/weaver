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

    pub fn translate(&mut self, x: f32, y: f32, z: f32) -> Self {
        self.matrix *= glam::Mat4::from_translation(glam::Vec3::new(x, y, z));
        *self
    }

    pub fn rotate(&mut self, angle: f32, axis: glam::Vec3) -> Self {
        self.matrix *= glam::Mat4::from_axis_angle(axis, angle);
        *self
    }

    pub fn scale(&mut self, x: f32, y: f32, z: f32) -> Self {
        self.matrix *= glam::Mat4::from_scale(glam::Vec3::new(x, y, z));
        *self
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}
