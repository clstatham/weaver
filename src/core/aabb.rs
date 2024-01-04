use super::transform::Transform;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: glam::Vec3,
    pub max: glam::Vec3,
}

impl Aabb {
    pub fn new(min: glam::Vec3, max: glam::Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_points(points: &[glam::Vec3]) -> Self {
        let mut min = glam::Vec3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = glam::Vec3::new(f32::MIN, f32::MIN, f32::MIN);

        for point in points {
            min = min.min(*point);
            max = max.max(*point);
        }

        Self { min, max }
    }

    pub fn union(&self, other: &Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    pub fn contains(&self, point: &glam::Vec3) -> bool {
        self.min.x <= point.x
            && self.min.y <= point.y
            && self.min.z <= point.z
            && self.max.x >= point.x
            && self.max.y >= point.y
            && self.max.z >= point.z
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    pub fn intersect_ray(&self, origin: glam::Vec3, direction: glam::Vec3) -> Option<f32> {
        let mut tmin = (self.min.x - origin.x) / direction.x;
        let mut tmax = (self.max.x - origin.x) / direction.x;

        if tmin > tmax {
            std::mem::swap(&mut tmin, &mut tmax);
        }

        let mut tymin = (self.min.y - origin.y) / direction.y;
        let mut tymax = (self.max.y - origin.y) / direction.y;

        if tymin > tymax {
            std::mem::swap(&mut tymin, &mut tymax);
        }

        if tmin > tymax || tymin > tmax {
            return None;
        }

        if tymin > tmin {
            tmin = tymin;
        }

        if tymax < tmax {
            tmax = tymax;
        }

        let mut tzmin = (self.min.z - origin.z) / direction.z;
        let mut tzmax = (self.max.z - origin.z) / direction.z;

        if tzmin > tzmax {
            std::mem::swap(&mut tzmin, &mut tzmax);
        }

        if tmin > tzmax || tzmin > tmax {
            return None;
        }

        if tzmin > tmin {
            tmin = tzmin;
        }

        if tzmax < tmax {
            tmax = tzmax;
        }

        if tmin < 0.0 && tmax < 0.0 {
            return None;
        }

        if tmin < 0.0 {
            return Some(tmax);
        }

        Some(tmin)
    }

    pub fn center(&self) -> glam::Vec3 {
        (self.min + self.max) / 2.0
    }

    pub fn transformed(&self, transform: Transform) -> Self {
        let transform = transform.matrix;
        let points = [
            transform.transform_point3(self.min),
            transform.transform_point3(glam::Vec3::new(self.min.x, self.min.y, self.max.z)),
            transform.transform_point3(glam::Vec3::new(self.min.x, self.max.y, self.min.z)),
            transform.transform_point3(glam::Vec3::new(self.min.x, self.max.y, self.max.z)),
            transform.transform_point3(glam::Vec3::new(self.max.x, self.min.y, self.min.z)),
            transform.transform_point3(glam::Vec3::new(self.max.x, self.min.y, self.max.z)),
            transform.transform_point3(glam::Vec3::new(self.max.x, self.max.y, self.min.z)),
            transform.transform_point3(self.max),
        ];

        Self::from_points(&points)
    }
}
