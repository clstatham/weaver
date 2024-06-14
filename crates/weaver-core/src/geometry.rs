use glam::*;
use weaver_ecs::prelude::Reflect;

use crate::prelude::Transform;

pub trait Intersect<Rhs> {
    type Output;
    fn intersect(&self, rhs: Rhs) -> Option<Self::Output>;
}

/// 3D plane with infinite extent
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

/// 3D ray with origin and direction, and infinite extent
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

/// Axis-aligned bounding box
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

impl Intersect<Aabb> for Ray {
    type Output = f32;

    fn intersect(&self, aabb: Aabb) -> Option<Self::Output> {
        let inv_direction = self.direction.recip();

        let t1 = (aabb.min - self.origin) * inv_direction;
        let t2 = (aabb.max - self.origin) * inv_direction;

        let tmin = t1.min(t2);
        let tmax = t1.max(t2);

        let tmin = tmin.max_element();
        let tmax = tmax.min_element();

        if tmin > tmax {
            return None;
        }

        let t = if tmin >= 0.0 {
            tmin
        } else if tmax >= 0.0 {
            tmax
        } else {
            return None;
        };

        Some(t)
    }
}

impl Intersect<Ray> for Aabb {
    type Output = <Ray as Intersect<Aabb>>::Output;

    fn intersect(&self, ray: Ray) -> Option<Self::Output> {
        ray.intersect(*self)
    }
}

impl Intersect<Plane> for Ray {
    type Output = f32;

    fn intersect(&self, plane: Plane) -> Option<Self::Output> {
        let denom = plane.normal.dot(self.direction);

        if denom.abs() < 1e-6 {
            return None;
        }

        let t = (plane.center - self.origin).dot(plane.normal) / denom;

        if t < 0.0 {
            return None;
        }

        Some(t)
    }
}

impl Intersect<Ray> for Plane {
    type Output = <Ray as Intersect<Plane>>::Output;

    fn intersect(&self, ray: Ray) -> Option<Self::Output> {
        ray.intersect(*self)
    }
}

/// Oriented bounding box
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
pub struct Obb {
    pub aabb: Aabb,
    pub transform: Transform,
}

impl Obb {
    pub fn new(aabb: Aabb, transform: Transform) -> Self {
        Self { aabb, transform }
    }

    pub fn center(&self) -> Vec3 {
        self.transform.translation
    }

    pub fn size(&self) -> Vec3 {
        self.aabb.size()
    }

    pub fn half_size(&self) -> Vec3 {
        self.aabb.half_size()
    }

    pub fn corners(&self) -> [Vec3; 8] {
        self.aabb.corners()
    }

    pub fn transform(&self, transform: Transform) -> Self {
        let matrix = transform.matrix();
        let aabb = self.aabb.transform(transform);
        let transform = Transform::from_matrix(matrix * self.transform.matrix());
        Self { aabb, transform }
    }
}

impl From<Aabb> for Obb {
    fn from(aabb: Aabb) -> Self {
        Self {
            aabb,
            transform: Transform::IDENTITY,
        }
    }
}

impl Intersect<Obb> for Ray {
    type Output = f32;

    fn intersect(&self, obb: Obb) -> Option<Self::Output> {
        let min = obb.aabb.min;
        let max = obb.aabb.max;

        let matrix = obb.transform.matrix();

        let mut tmin = 0.0f32;
        let mut tmax = f32::INFINITY;

        let obb_pos_worldspace = matrix.col(3).truncate();

        let delta = obb_pos_worldspace - self.origin;

        for i in 0..3 {
            let axis = matrix.col(i).truncate();
            let e = axis.dot(delta);
            let f = axis.dot(self.direction);

            if f.abs() > 1e-6 {
                let mut t1 = (e + min[i]) / f;
                let mut t2 = (e + max[i]) / f;

                if t1 > t2 {
                    std::mem::swap(&mut t1, &mut t2);
                }

                tmin = tmin.max(t1);
                tmax = tmax.min(t2);

                if tmin > tmax {
                    return None;
                }
            } else if -e + min[i] > 0.0 || -e + max[i] < 0.0 {
                return None;
            }
        }

        if tmin < 0.0 {
            Some(tmax)
        } else {
            Some(tmin)
        }
    }
}

impl Intersect<Ray> for Obb {
    type Output = <Ray as Intersect<Obb>>::Output;

    fn intersect(&self, ray: Ray) -> Option<Self::Output> {
        ray.intersect(*self)
    }
}
