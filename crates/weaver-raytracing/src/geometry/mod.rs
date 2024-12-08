use encase::ShaderType;
use weaver_core::prelude::Vec3;

pub trait Geometry: 'static + ShaderType {}

#[derive(Debug, Clone, Copy, PartialEq, ShaderType)]
pub struct Plane {
    pub normal: Vec3,
    pub distance: f32,
}

impl Geometry for Plane {}

#[derive(Debug, Clone, Copy, PartialEq, ShaderType)]
pub struct Sphere {
    pub radius: f32,
}

impl Geometry for Sphere {}
