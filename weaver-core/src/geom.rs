use super::transform::GlobalTransform;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    pub fn center(&self) -> glam::Vec2 {
        glam::Vec2::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn top_left(&self) -> glam::Vec2 {
        glam::Vec2::new(self.x, self.y)
    }

    pub fn top_right(&self) -> glam::Vec2 {
        glam::Vec2::new(self.x + self.width, self.y)
    }

    pub fn bottom_left(&self) -> glam::Vec2 {
        glam::Vec2::new(self.x, self.y + self.height)
    }

    pub fn bottom_right(&self) -> glam::Vec2 {
        glam::Vec2::new(self.x + self.width, self.y + self.height)
    }

    pub fn from_points(points: &[glam::Vec2]) -> Self {
        let mut min = glam::Vec2::new(f32::MAX, f32::MAX);
        let mut max = glam::Vec2::new(f32::MIN, f32::MIN);

        for point in points {
            min = min.min(*point);
            max = max.max(*point);
        }

        Self {
            x: min.x,
            y: min.y,
            width: max.x - min.x,
            height: max.y - min.y,
        }
    }
}

impl From<egui::Rect> for Rect {
    fn from(rect: egui::Rect) -> Self {
        Self {
            x: rect.min.x,
            y: rect.min.y,
            width: rect.width(),
            height: rect.height(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: glam::Vec3,
    pub direction: glam::Vec3,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BoundingSphere {
    pub center: glam::Vec3,
    pub radius: f32,
}

impl BoundingSphere {
    pub fn new(center: glam::Vec3, radius: f32) -> Self {
        Self { center, radius }
    }

    pub fn from_points(points: &[glam::Vec3]) -> Self {
        let center = points.iter().fold(glam::Vec3::ZERO, |acc, p| acc + *p) / points.len() as f32;
        let radius = points
            .iter()
            .map(|p| (*p - center).length())
            .fold(0.0f32, |acc, r| acc.max(r));

        Self { center, radius }
    }

    pub fn union(&self, other: &Self) -> Self {
        let center = (self.center + other.center) / 2.0;
        let radius = (self.center - other.center).length() + self.radius + other.radius;

        Self { center, radius }
    }

    pub fn contains(&self, point: &glam::Vec3) -> bool {
        (*point - self.center).length_squared() <= self.radius * self.radius
    }

    pub fn intersects(&self, other: &Self) -> bool {
        (self.center - other.center).length_squared()
            <= (self.radius + other.radius) * (self.radius + other.radius)
    }

    pub fn intersect_ray(&self, ray: Ray) -> Option<f32> {
        let l = self.center - ray.origin;
        let tca = l.dot(ray.direction);
        let d2 = l.dot(l) - tca * tca;

        if d2 > self.radius * self.radius {
            return None;
        }

        let thc = (self.radius * self.radius - d2).sqrt();

        let t0 = tca - thc;
        let t1 = tca + thc;

        if t0 < 0.0 && t1 < 0.0 {
            return None;
        }

        if t0 < 0.0 {
            return Some(t1);
        }

        Some(t0)
    }

    pub fn transformed(&self, transform: GlobalTransform) -> Self {
        let transform = transform.matrix;
        let points = [
            transform.transform_point3(self.center),
            transform.transform_point3(glam::Vec3::new(
                self.center.x,
                self.center.y,
                self.center.z + self.radius,
            )),
            transform.transform_point3(glam::Vec3::new(
                self.center.x,
                self.center.y,
                self.center.z - self.radius,
            )),
            transform.transform_point3(glam::Vec3::new(
                self.center.x,
                self.center.y + self.radius,
                self.center.z,
            )),
            transform.transform_point3(glam::Vec3::new(
                self.center.x,
                self.center.y - self.radius,
                self.center.z,
            )),
            transform.transform_point3(glam::Vec3::new(
                self.center.x + self.radius,
                self.center.y,
                self.center.z,
            )),
            transform.transform_point3(glam::Vec3::new(
                self.center.x - self.radius,
                self.center.y,
                self.center.z,
            )),
        ];

        Self::from_points(&points)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
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

    pub fn transformed(&self, transform: GlobalTransform) -> Self {
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
