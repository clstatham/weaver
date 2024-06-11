use glam::*;
use weaver_reflect::prelude::Reflect;

use crate::prelude::Transform;

#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[repr(C)]
pub struct Plane {
    pub normal: Vec3,
    pub center: Vec3,
}

impl Plane {
    pub fn new(normal: Vec3, center: Vec3) -> Self {
        Self { normal, center }
    }

    pub fn from_points(a: Vec3, b: Vec3, c: Vec3) -> Self {
        let normal = (b - a).cross(c - a).normalize();
        let center = a;
        Self { normal, center }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[repr(C)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self { origin, direction }
    }

    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[repr(C)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) / 2.0
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    pub fn half_size(&self) -> Vec3 {
        self.size() / 2.0
    }

    pub fn transform(&self, transform: Transform) -> Self {
        let matrix = transform.matrix();
        let min = matrix.transform_point3(self.min);
        let max = matrix.transform_point3(self.max);
        Self { min, max }
    }

    pub fn corners(&self) -> [Vec3; 8] {
        [
            Vec3::new(self.min.x, self.min.y, self.min.z),
            Vec3::new(self.min.x, self.min.y, self.max.z),
            Vec3::new(self.min.x, self.max.y, self.min.z),
            Vec3::new(self.min.x, self.max.y, self.max.z),
            Vec3::new(self.max.x, self.min.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.min.z),
            Vec3::new(self.max.x, self.max.y, self.max.z),
        ]
    }
}
