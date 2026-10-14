//! Position, orientation and scale of an entity.

use glam::*;

/// Translation, rotation and scale of something in world space.
///
/// There's no parent/child hierarchy: a `Transform` is always the final world-space pose. Applied
/// in scale, rotation, translation order (see [`Transform::matrix`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Position in world space.
    pub translation: Vec3,
    /// Orientation.
    pub rotation: Quat,
    /// Per-axis scale, applied before the rotation.
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    /// No translation, no rotation, unit scale.
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    /// Creates a transform from its three parts.
    pub fn new(translation: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            translation,
            rotation,
            scale,
        }
    }

    /// Creates a transform that only translates.
    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    /// Creates a transform that only rotates.
    pub fn from_rotation(rotation: Quat) -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation,
            scale: Vec3::ONE,
        }
    }

    /// Creates a transform that only scales.
    pub fn from_scale(scale: Vec3) -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale,
        }
    }

    /// Returns the right-handed view transform for a camera at `eye` looking at `target`.
    ///
    /// This is the world-to-camera matrix ([`Mat4::look_at_rh`]) decomposed into a transform, so
    /// it's the inverse of the camera's pose. Call [`Transform::inverse`] on it to get the pose.
    // FIXME: returns the view matrix (inverse of the camera pose), not a pose.
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let matrix = Mat4::look_at_rh(eye, target, up);
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();

        Self {
            translation,
            rotation,
            scale,
        }
    }

    /// Decomposes a perspective projection matrix into a transform.
    ///
    /// A projection isn't an affine transform, so this isn't a meaningful pose.
    // FIXME: uses perspective_rh_gl (GL depth range) but wgpu clip depth is 0..1.
    pub fn perspective(fov: f32, aspect: f32, near: f32, far: f32) -> Self {
        let matrix = Mat4::perspective_rh_gl(fov, aspect, near, far);
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();

        Self {
            translation,
            rotation,
            scale,
        }
    }

    /// Returns the 4x4 matrix for this transform (scale, then rotation, then translation).
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.scale,
            Quat::from_array(self.rotation.to_array()),
            self.translation,
        )
    }

    /// Decomposes an affine matrix into a transform.
    ///
    /// Shear and projection parts of the matrix are dropped.
    pub fn from_matrix(matrix: Mat4) -> Self {
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();

        Self {
            translation,
            rotation,
            scale,
        }
    }

    /// Returns the transform that undoes this one.
    ///
    /// Goes through the matrix inverse and decomposes again, so it isn't exact when the scale is
    /// non-uniform and the rotation isn't the identity.
    pub fn inverse(&self) -> Self {
        self.matrix().inverse().into()
    }

    /// Returns the inverse of [`Transform::matrix`].
    pub fn inverse_matrix(&self) -> Mat4 {
        self.matrix().inverse()
    }

    /// Transforms `point` as a position (scale, rotation and translation apply).
    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        self.matrix().transform_point3(point)
    }

    /// Transforms `vector` as a direction (translation is ignored).
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
