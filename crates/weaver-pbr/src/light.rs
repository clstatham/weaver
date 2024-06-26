use encase::ShaderType;
use weaver_app::{plugin::Plugin, App};

use weaver_core::{color::Color, prelude::Vec3};
use weaver_ecs::{
    prelude::{Component, Reflect, Resource, World},
    world::FromWorld,
};
use weaver_renderer::{
    bind_group::{BindGroupLayout, CreateBindGroup, ResourceBindGroupPlugin},
    buffer::GpuBufferVec,
    prelude::*,
    WgpuDevice,
};
use weaver_util::prelude::Result;

#[derive(Copy, Clone, Debug, Component, Reflect)]
pub struct PointLight {
    pub color: Color,
    pub intensity: f32,
    pub radius: f32,
    pub enabled: bool,
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

#[derive(Resource)]
pub struct GpuPointLightArray {
    pub buffer: GpuBufferVec<PointLightUniform>,
}

impl FromWorld for GpuPointLightArray {
    fn from_world(world: &mut World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();
        let mut buffer =
            GpuBufferVec::new(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        buffer.reserve(1, &device);
        Self { buffer }
    }
}

impl CreateBindGroup for GpuPointLightArray {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Point Light Bind Group Layout"),
        })
    }

    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        cached_layout: &BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: cached_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.buffer.binding().unwrap(),
            }],
            label: Some("Point Light Bind Group"),
        })
    }
}

pub struct PointLightPlugin;

impl Plugin for PointLightPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_plugin(ResourceBindGroupPlugin::<GpuPointLightArray>::default())?;

        Ok(())
    }
}
