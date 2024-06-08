use glam::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn new(translation: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            translation,
            rotation,
            scale,
        }
    }

    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    pub fn from_rotation(rotation: Quat) -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation,
            scale: Vec3::ONE,
        }
    }

    pub fn from_scale(scale: Vec3) -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale,
        }
    }

    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let matrix = Mat4::look_at_rh(eye, target, up);
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();

        Self {
            translation,
            rotation,
            scale,
        }
    }

    pub fn perspective(fov: f32, aspect: f32, near: f32, far: f32) -> Self {
        let matrix = Mat4::perspective_rh(fov, aspect, near, far);
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();

        Self {
            translation,
            rotation,
            scale,
        }
    }

    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    pub fn from_matrix(matrix: Mat4) -> Self {
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();

        Self {
            translation,
            rotation,
            scale,
        }
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
