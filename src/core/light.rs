use crate::ecs::component::Component;

use super::color::Color;

#[derive(Debug, Clone, Copy)]
pub enum Light {
    Point(PointLight),
    Directional(DirectionalLight),
    Spot(SpotLight),
}
impl Component for Light {}

#[derive(Debug, Clone, Copy)]
pub struct PointLight {
    pub position: glam::Vec3A,
    pub color: Color,
    pub intensity: f32,
}
impl Component for PointLight {}

impl PointLight {
    pub fn new(position: glam::Vec3A, color: Color, intensity: f32) -> Self {
        Self {
            position,
            color,
            intensity,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DirectionalLight {
    pub direction: glam::Vec3A,
    pub color: Color,
    pub intensity: f32,
}
impl Component for DirectionalLight {}

impl DirectionalLight {
    pub fn new(direction: glam::Vec3A, color: Color, intensity: f32) -> Self {
        Self {
            direction,
            color,
            intensity,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SpotLight {
    pub position: glam::Vec3A,
    pub direction: glam::Vec3A,
    pub color: Color,
    pub intensity: f32,
    pub angle: f32,
}
impl Component for SpotLight {}

impl SpotLight {
    pub fn new(
        position: glam::Vec3A,
        direction: glam::Vec3A,
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
