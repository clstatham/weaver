use glam::*;
use weaver_ecs::prelude::Reflect;

use crate::{mesh::Mesh, prelude::Transform};

pub trait Intersect<Rhs> {
    type Output;
    fn intersect(&self, rhs: &Rhs) -> Option<Self::Output>;
}

/// 3D plane with infinite extent
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[repr(C)]
pub struct Plane {
    pub normal: Vec3A,
    pub center: Vec3A,
}

impl Plane {
    pub fn new(normal: Vec3A, center: Vec3A) -> Self {
        Self { normal, center }
    }

    pub fn from_points(a: Vec3A, b: Vec3A, c: Vec3A) -> Self {
        let normal = (b - a).cross(c - a).normalize();
        let center = a;
        Self { normal, center }
    }

    pub fn transformed(&self, transform: Transform) -> Self {
        let matrix = transform.matrix();
        let normal = matrix.transform_vector3a(self.normal);
        let center = matrix.transform_point3a(self.center);
        Self { normal, center }
    }
}

/// 3D ray with origin and direction, and infinite extent
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[repr(C)]
pub struct Ray {
    pub origin: Vec3A,
    pub direction: Vec3A,
}

impl Ray {
    pub fn new(origin: Vec3A, direction: Vec3A) -> Self {
        Self { origin, direction }
    }

    pub fn at(&self, t: f32) -> Vec3A {
        self.origin + self.direction * t
    }

    pub fn transformed(&self, transform: Transform) -> Self {
        let matrix = transform.matrix();
        Self {
            origin: matrix.transform_point3a(self.origin),
            direction: matrix.transform_vector3a(self.direction),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[repr(C)]
pub struct Triangle {
    pub a: Vec3A,
    pub b: Vec3A,
    pub c: Vec3A,
}

impl Triangle {
    pub fn new(a: Vec3A, b: Vec3A, c: Vec3A) -> Self {
        Self { a, b, c }
    }

    pub fn normal(&self) -> Vec3A {
        (self.b - self.a).cross(self.c - self.a).normalize()
    }

    pub fn transformed(&self, transform: Transform) -> Self {
        let matrix = transform.matrix();
        Self {
            a: matrix.transform_point3a(self.a),
            b: matrix.transform_point3a(self.b),
            c: matrix.transform_point3a(self.c),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
pub struct RayTriangleIntersection {
    pub t: f32,
    pub uv: Vec2,
}

impl Intersect<Ray> for Triangle {
    type Output = RayTriangleIntersection;

    fn intersect(&self, ray: &Ray) -> Option<Self::Output> {
        let edge1 = self.b - self.a;
        let edge2 = self.c - self.a;
        let h = ray.direction.cross(edge2);
        let a = edge1.dot(h);

        if a > -1e-6 && a < 1e-6 {
            return None;
        }

        let f = 1.0 / a;
        let s = ray.origin - self.a;
        let u = f * s.dot(h);

        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let q = s.cross(edge1);
        let v = f * ray.direction.dot(q);

        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = f * edge2.dot(q);

        if t > 1e-6 {
            let uv = Vec2::new(u, v);
            Some(RayTriangleIntersection { t, uv })
        } else {
            None
        }
    }
}

impl Intersect<Triangle> for Ray {
    type Output = <Triangle as Intersect<Ray>>::Output;

    fn intersect(&self, triangle: &Triangle) -> Option<Self::Output> {
        triangle.intersect(self)
    }
}

/// Axis-aligned bounding box
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[repr(C)]
pub struct Aabb {
    pub min: Vec3A,
    pub max: Vec3A,
}

impl Aabb {
    pub fn new(min: Vec3A, max: Vec3A) -> Self {
        Self { min, max }
    }

    pub fn center(&self) -> Vec3A {
        (self.min + self.max) / 2.0
    }

    pub fn size(&self) -> Vec3A {
        self.max - self.min
    }

    pub fn half_size(&self) -> Vec3A {
        self.size() / 2.0
    }

    pub fn transformed(&self, transform: Transform) -> Self {
        let matrix = transform.matrix();
        let min = matrix.transform_point3a(self.min);
        let max = matrix.transform_point3a(self.max);
        Self { min, max }
    }

    pub fn corners(&self) -> [Vec3A; 8] {
        [
            Vec3A::new(self.min.x, self.min.y, self.min.z),
            Vec3A::new(self.min.x, self.min.y, self.max.z),
            Vec3A::new(self.min.x, self.max.y, self.min.z),
            Vec3A::new(self.min.x, self.max.y, self.max.z),
            Vec3A::new(self.max.x, self.min.y, self.min.z),
            Vec3A::new(self.max.x, self.min.y, self.max.z),
            Vec3A::new(self.max.x, self.max.y, self.min.z),
            Vec3A::new(self.max.x, self.max.y, self.max.z),
        ]
    }
}

impl Intersect<Aabb> for Ray {
    type Output = f32;

    fn intersect(&self, aabb: &Aabb) -> Option<Self::Output> {
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

    fn intersect(&self, ray: &Ray) -> Option<Self::Output> {
        ray.intersect(self)
    }
}

impl Intersect<Plane> for Ray {
    type Output = f32;

    fn intersect(&self, plane: &Plane) -> Option<Self::Output> {
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

    fn intersect(&self, ray: &Ray) -> Option<Self::Output> {
        ray.intersect(self)
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

    pub fn center(&self) -> Vec3A {
        self.transform.translation
    }

    pub fn size(&self) -> Vec3A {
        self.aabb.size()
    }

    pub fn half_size(&self) -> Vec3A {
        self.aabb.half_size()
    }

    pub fn corners(&self) -> [Vec3A; 8] {
        self.aabb.corners()
    }

    pub fn transformed(&self, transform: Transform) -> Self {
        let matrix = transform.matrix();
        let aabb = self.aabb.transformed(transform);
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

    fn intersect(&self, obb: &Obb) -> Option<Self::Output> {
        let min = obb.aabb.min;
        let max = obb.aabb.max;

        let matrix = obb.transform.matrix();

        let mut tmin = 0.0f32;
        let mut tmax = f32::INFINITY;

        let obb_pos_worldspace: Vec3A = matrix.col(3).truncate().into();

        let delta = obb_pos_worldspace - self.origin;

        for i in 0..3 {
            let axis: Vec3A = matrix.col(i).truncate().into();
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

    fn intersect(&self, ray: &Ray) -> Option<Self::Output> {
        ray.intersect(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
pub struct RayMeshIntersection {
    pub ray_triangle_intersection: RayTriangleIntersection,
    pub triangle: Triangle,
}

impl Intersect<Ray> for Mesh {
    type Output = RayMeshIntersection;

    /// Assumes the mesh has already been transformed into world space.
    fn intersect(&self, rhs: &Ray) -> Option<Self::Output> {
        let mut closest_intersection: Option<RayMeshIntersection> = None;

        for indices in self.indices.chunks(3) {
            let a = self.vertices[indices[0] as usize].position;
            let b = self.vertices[indices[1] as usize].position;
            let c = self.vertices[indices[2] as usize].position;

            let triangle = Triangle::new(a.into(), b.into(), c.into());

            if let Some(intersection) = rhs.intersect(&triangle) {
                if let Some(ref closest) = closest_intersection {
                    if intersection.t < closest.ray_triangle_intersection.t {
                        closest_intersection = Some(RayMeshIntersection {
                            ray_triangle_intersection: intersection,
                            triangle,
                        });
                    }
                } else {
                    closest_intersection = Some(RayMeshIntersection {
                        ray_triangle_intersection: intersection,
                        triangle,
                    });
                }
            }
        }

        closest_intersection
    }
}

impl Intersect<Mesh> for Ray {
    type Output = <Mesh as Intersect<Ray>>::Output;

    fn intersect(&self, mesh: &Mesh) -> Option<Self::Output> {
        mesh.intersect(self)
    }
}
