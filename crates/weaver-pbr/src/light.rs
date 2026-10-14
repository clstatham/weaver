//! Point lights: the main-world component and the render-world types built from it.

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

/// A light radiating from a single point.
///
/// The position is the entity's [`Transform`] translation, or the origin if it has none.
#[derive(Copy, Clone, Debug)]
pub struct PointLight {
    /// Light color.
    pub color: Color,
    /// Brightness multiplier on `color`.
    pub intensity: f32,
    /// Size of the light, passed to the shader.
    pub radius: f32,
    /// Disabled lights are uploaded with zero intensity.
    pub enabled: bool,
}

/// A point light in the render world, with its main-world position.
#[derive(Copy, Clone, Debug)]
pub struct ExtractedPointLight {
    /// The main-world component.
    pub light: PointLight,
    /// World-space position.
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

/// A [`PointLight`] as the shader's storage buffer element (48 bytes).
#[derive(Copy, Clone, Debug, Default, ShaderType)]
#[repr(C)]
pub struct PointLightUniform {
    /// World-space position.
    pub position: Vec3,
    _padding: u32,
    /// Light color.
    pub color: Color,
    /// Brightness multiplier on `color`; zero for disabled lights.
    pub intensity: f32,
    /// Size of the light.
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

/// The storage buffer of every point light in the scene, bound in the lighting bind group.
pub struct GpuPointLightArray {
    /// Rebuilt every frame; never empty (see [`PointLightUniform::NONE`]).
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

/// Extracts [`PointLight`]s into the render world as [`ExtractedPointLight`]s.
pub struct PointLightPlugin;

impl Plugin for PointLightPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_plugin(ExtractComponentPlugin::<PointLight>::default())?;

        Ok(())
    }
}
