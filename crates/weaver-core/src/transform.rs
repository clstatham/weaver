use glam::*;
use weaver_ecs::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Reflect, Component)]
pub struct Transform {
    pub translation: Vec3A,
    pub rotation: Quat,
    pub scale: Vec3A,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Self = Self {
        translation: Vec3A::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3A::ONE,
    };

    pub fn new(translation: Vec3A, rotation: Quat, scale: Vec3A) -> Self {
        Self {
            translation,
            rotation,
            scale,
        }
    }

    pub fn from_translation(translation: Vec3A) -> Self {
        Self {
            translation,
            rotation: Quat::IDENTITY,
            scale: Vec3A::ONE,
        }
    }

    pub fn from_rotation(rotation: Quat) -> Self {
        Self {
            translation: Vec3A::ZERO,
            rotation,
            scale: Vec3A::ONE,
        }
    }

    pub fn from_scale(scale: Vec3A) -> Self {
        Self {
            translation: Vec3A::ZERO,
            rotation: Quat::IDENTITY,
            scale,
        }
    }

    pub fn look_at(eye: Vec3A, target: Vec3A, up: Vec3A) -> Self {
        let matrix = Mat4::look_at_rh(eye.into(), target.into(), up.into());
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();

        Self {
            translation: translation.into(),
            rotation,
            scale: scale.into(),
        }
    }

    pub fn perspective(fov: f32, aspect: f32, near: f32, far: f32) -> Self {
        let matrix = Mat4::perspective_rh(fov, aspect, near, far);
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();

        Self {
            translation: translation.into(),
            rotation,
            scale: scale.into(),
        }
    }

    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.scale.into(),
            Quat::from_array(self.rotation.to_array()),
            self.translation.into(),
        )
    }

    pub fn from_matrix(matrix: Mat4) -> Self {
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();

        Self {
            translation: translation.into(),
            rotation,
            scale: scale.into(),
        }
    }

    pub fn inverse(&self) -> Self {
        self.matrix().inverse().into()
    }

    pub fn inverse_matrix(&self) -> Mat4 {
        self.matrix().inverse()
    }

    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        self.matrix().transform_point3(point)
    }

    pub fn transform_vector(&self, vector: Vec3) -> Vec3 {
        self.matrix().transform_vector3(vector)
    }
}

impl From<Transform> for Mat4 {
    fn from(transform: Transform) -> Self {
        transform.matrix()
    }
}

impl From<Mat4> for Transform {
    fn from(matrix: Mat4) -> Self {
        Transform::from_matrix(matrix)
    }
}
