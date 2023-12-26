use super::color::Color;

#[derive(Debug, Clone, Copy)]
pub enum Light {
    Point(PointLight),
    Directional(DirectionalLight),
    Spot(SpotLight),
}

#[derive(Debug, Clone, Copy)]
pub struct PointLight {
    pub position: glam::Vec3,
    pub color: Color,
    pub intensity: f32,
}

impl PointLight {
    pub fn new(position: glam::Vec3, color: Color, intensity: f32) -> Self {
        Self {
            position,
            color,
            intensity,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DirectionalLight {
    pub direction: glam::Vec3,
    pub color: Color,
    pub intensity: f32,
}

impl DirectionalLight {
    pub fn new(direction: glam::Vec3, color: Color, intensity: f32) -> Self {
        Self {
            direction,
            color,
            intensity,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SpotLight {
    pub position: glam::Vec3,
    pub direction: glam::Vec3,
    pub color: Color,
    pub intensity: f32,
    pub angle: f32,
}

impl SpotLight {
    pub fn new(
        position: glam::Vec3,
        direction: glam::Vec3,
        color: Color,
        intensity: f32,
        angle: f32,
    ) -> Self {
        Self {
            position,
            direction,
            color,
            intensity,
            angle,
        }
    }
}
