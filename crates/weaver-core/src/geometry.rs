use encase::ShaderType;
use glam::*;

use crate::{mesh::Mesh, prelude::Transform};

pub trait Intersect<Rhs> {
    type Output;
    fn intersect(&self, rhs: &Rhs) -> Option<Self::Output>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum Intersection {
    Inside = 1,
    Outside = 2,
    Intersecting = 3, // 1 | 2
}

/// 3D plane with infinite extent
#[derive(Default, Clone, Copy, Debug, PartialEq, ShaderType)]
#[repr(C)]
pub struct Plane {
    /// Normal vector of the plane
    pub normal: Vec3,
    /// Distance from the origin along the normal vector
    pub distance: f32,
}

impl Plane {
    #[inline]
    pub fn new(normal: Vec3, distance: f32) -> Self {
        Self { normal, distance }
    }

    #[inline]
    pub fn from_normal_and_point(normal: Vec3, point: Vec3) -> Self {
        let distance = normal.dot(point);
        Self { normal, distance }
    }

    #[inline]
    pub fn from_points(a: Vec3, b: Vec3, c: Vec3) -> Self {
        let normal = (b - a).cross(c - a).normalize();
        let center = a;
        let distance = normal.dot(center);
        Self { normal, distance }
    }

    #[inline]
    pub fn from_coefficients(a: f32, b: f32, c: f32, d: f32, normalize: bool) -> Self {
        let normal = Vec3::new(a, b, c);
        let distance = d;
        if normalize {
            let length_recip = normal.length_recip();
            let normal = normal.normalize();
            let distance = distance * length_recip;
            Self { normal, distance }
        } else {
            Self { normal, distance }
        }
    }

    #[inline]
    pub fn from_coefficient_vec4(coefficients: Vec4, normalize: bool) -> Self {
        Self::from_coefficients(
            coefficients.x,
            coefficients.y,
            coefficients.z,
            coefficients.w,
            normalize,
        )
    }

    #[inline]
    pub fn to_coefficients(&self) -> Vec4 {
        Vec4::new(self.normal.x, self.normal.y, self.normal.z, self.distance)
    }

    #[inline]
    pub fn center(&self) -> Vec3 {
        self.normal * self.distance
    }

    #[inline]
    pub fn transformed(&self, transform: Transform) -> Self {
        let matrix = transform.matrix();
        let normal = matrix.transform_vector3(self.normal);
        let center = matrix.transform_point3(self.center());
        let distance = normal.dot(center);
        Self { normal, distance }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HalfSpace {
    Front,
    Back,
    On,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, ShaderType)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

impl Sphere {
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }

    pub fn transformed(&self, transform: Transform) -> Self {
        let matrix = transform.matrix();
        let center = matrix.transform_point3(self.center);
        Self {
            center,
            radius: self.radius,
        }
    }
}

impl Intersect<Sphere> for Sphere {
    type Output = Intersection;

    fn intersect(&self, rhs: &Sphere) -> Option<Self::Output> {
        let distance_squared = (self.center - rhs.center).length_squared();
        let radius_sum = self.radius + rhs.radius;
        let radius_sum_squared = radius_sum * radius_sum;

        if distance_squared < radius_sum_squared {
            if distance_squared <= (self.radius - rhs.radius).powi(2) {
                Some(Intersection::Inside)
            } else {
                Some(Intersection::Intersecting)
            }
        } else {
            Some(Intersection::Outside)
        }
    }
}

/// 3D ray with origin and direction, and infinite extent
#[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
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

    pub fn transformed(&self, transform: Transform) -> Self {
        let matrix = transform.matrix();
        Self {
            origin: matrix.transform_point3(self.origin),
            direction: matrix.transform_vector3(self.direction),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
#[repr(C)]
pub struct Triangle {
    pub a: Vec3,
    pub b: Vec3,
    pub c: Vec3,
}

impl Triangle {
    pub fn new(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self { a, b, c }
    }

    pub fn normal(&self) -> Vec3 {
        (self.b - self.a).cross(self.c - self.a).normalize()
    }

    pub fn transformed(&self, transform: Transform) -> Self {
        let matrix = transform.matrix();
        Self {
            a: matrix.transform_point3(self.a),
            b: matrix.transform_point3(self.b),
            c: matrix.transform_point3(self.c),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
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

#[derive(Default, Clone, Copy, Debug, PartialEq, ShaderType)]
#[repr(C)]
pub struct Frustum {
    pub left: Plane,
    pub right: Plane,
    pub bottom: Plane,
    pub top: Plane,
    pub near: Plane,
    pub far: Plane,
}

impl Frustum {
    #[inline]
    pub fn from_view_proj(view_proj: Mat4) -> Self {
        let left = Plane::from_coefficient_vec4(view_proj.row(3) + view_proj.row(0), true);
        let right = Plane::from_coefficient_vec4(view_proj.row(3) - view_proj.row(0), true);
        let bottom = Plane::from_coefficient_vec4(view_proj.row(3) + view_proj.row(1), true);
        let top = Plane::from_coefficient_vec4(view_proj.row(3) - view_proj.row(1), true);
        let near = Plane::from_coefficient_vec4(view_proj.row(3) + view_proj.row(2), true);
        let far = Plane::from_coefficient_vec4(view_proj.row(3) - view_proj.row(2), true);
        Self {
            left,
            right,
            bottom,
            top,
            near,
            far,
        }
    }

    #[inline]
    pub fn planes(&self) -> [Plane; 6] {
        [
            self.left,
            self.right,
            self.bottom,
            self.top,
            self.near,
            self.far,
        ]
    }

    #[inline]
    pub fn bounding_sphere(&self) -> Sphere {
        let mut center = Vec3::ZERO;
        for plane in self.planes().iter() {
            center += plane.normal * plane.distance;
        }
        center /= 6.0;

        let mut radius = 0.0f32;
        for plane in self.planes().iter() {
            let distance = plane.normal.dot(center) - plane.distance;
            radius = radius.max(distance.abs());
        }

        Sphere::new(center, radius)
    }
}

/// Axis-aligned bounding box
#[derive(Clone, Copy, Debug, PartialEq, Default, ShaderType)]
#[repr(C)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    #[inline]
    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    #[inline]
    pub fn center(&self) -> Vec3 {
        (self.max + self.min) * 0.5
    }

    #[inline]
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    #[inline]
    pub fn half_size(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    #[inline]
    pub fn relative_radius(&self, plane_normal: Vec3) -> f32 {
        self.half_size().dot(plane_normal.abs())
    }

    #[inline]
    pub fn transformed(&self, transform: Transform) -> Self {
        let matrix = transform.matrix();
        let min = matrix.transform_point3(self.min);
        let max = matrix.transform_point3(self.max);
        Self { min, max }
    }

    pub const fn corners(&self) -> [Vec3; 8] {
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

    #[inline]
    pub fn bounding_sphere(&self) -> Sphere {
        let center = self.center();
        let radius = self.size().length() * 0.5;
        Sphere::new(center, radius)
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

impl Intersect<Sphere> for Aabb {
    type Output = Intersection;

    fn intersect(&self, sphere: &Sphere) -> Option<Self::Output> {
        let mut distance_squared = 0.0;
        for i in 0..3 {
            let v = sphere.center[i];
            if v < self.min[i] {
                distance_squared += (self.min[i] - v).powi(2);
            } else if v > self.max[i] {
                distance_squared += (v - self.max[i]).powi(2);
            }
        }

        let radius = sphere.radius;
        let radius_squared = radius * radius;

        if distance_squared <= radius_squared {
            if self.min.x <= sphere.center.x
                && sphere.center.x <= self.max.x
                && self.min.y <= sphere.center.y
                && sphere.center.y <= self.max.y
                && self.min.z <= sphere.center.z
                && sphere.center.z <= self.max.z
            {
                Some(Intersection::Inside)
            } else {
                Some(Intersection::Intersecting)
            }
        } else {
            Some(Intersection::Outside)
        }
    }
}

/// Oriented bounding box
#[derive(Clone, Copy, Debug, PartialEq)]
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

        let obb_pos_worldspace: Vec3 = matrix.col(3).truncate();

        let delta = obb_pos_worldspace - self.origin;

        for i in 0..3 {
            let axis: Vec3 = matrix.col(i).truncate();
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

#[derive(Clone, Copy, Debug, PartialEq)]
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

            let triangle = Triangle::new(a, b, c);

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
