use encase::ShaderType;
use weaver_app::{App, plugin::Plugin};

use weaver_core::{color::Color, prelude::Vec3, transform::Transform};
use weaver_ecs::{
    query::QueryableItem,
    world::{ConstructFromWorld, World},
};
use weaver_renderer::{
    WgpuDevice, WgpuQueue, buffer::GpuBufferVec, extract::ExtractComponentPlugin, prelude::*,
};
use weaver_util::prelude::*;

#[derive(Copy, Clone, Debug)]
pub struct PointLight {
    pub color: Color,
    pub intensity: f32,
    pub radius: f32,
    pub enabled: bool,
}

/// A point light in the render world, with its main-world position.
#[derive(Copy, Clone, Debug)]
pub struct ExtractedPointLight {
    pub light: PointLight,
    pub position: Vec3,
}

impl ExtractComponent for PointLight {
    type Data = (&'static Self, Option<&'static Transform>);
    // the light's transform can change without the light changing
    type Filter = ();
    type Out = ExtractedPointLight;

    fn extract((light, transform): QueryableItem<Self::Data>) -> Option<Self::Out> {
        Some(ExtractedPointLight {
            light: *light,
            position: transform.map_or(Vec3::ZERO, |transform| transform.translation),
        })
    }
}

#[derive(Copy, Clone, Debug, Default, ShaderType)]
#[repr(C)]
pub struct PointLightUniform {
    pub position: Vec3,
    _padding: u32,
    pub color: Color,
    pub intensity: f32,
    pub radius: f32,
}

impl PointLightUniform {
    /// A light that contributes nothing, uploaded when there are no lights, since the light
    /// buffer can't be bound empty.
    pub const NONE: Self = Self {
        position: Vec3::ZERO,
        _padding: 0,
        color: Color::BLACK,
        intensity: 0.0,
        radius: 0.0,
    };
}

impl From<ExtractedPointLight> for PointLightUniform {
    fn from(ExtractedPointLight { light, position }: ExtractedPointLight) -> Self {
        Self {
            position,
            _padding: 0,
            color: light.color,
            intensity: if light.enabled { light.intensity } else { 0.0 },
            radius: light.radius,
        }
    }
}

pub struct GpuPointLightArray {
    pub buffer: GpuBufferVec<PointLightUniform>,
}

impl ConstructFromWorld for GpuPointLightArray {
    fn from_world(world: &World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();
        let queue = world.get_resource::<WgpuQueue>().unwrap();
        let mut buffer =
            GpuBufferVec::new(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        buffer.push(PointLightUniform::NONE);
        buffer.enqueue_update(&device, &queue);
        Self { buffer }
    }
}

pub struct PointLightPlugin;

impl Plugin for PointLightPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_plugin(ExtractComponentPlugin::<PointLight>::default())?;

        Ok(())
    }
}
