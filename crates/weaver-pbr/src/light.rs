use weaver_app::{plugin::Plugin, App};
use wgpu::util::DeviceExt;

use weaver_core::{color::Color, prelude::Vec3, transform::Transform};
use weaver_ecs::prelude::{Component, Reflect, Resource, World};
use weaver_renderer::{
    bind_group::{BindGroup, BindGroupLayout, CreateBindGroup, ResourceBindGroupPlugin},
    buffer::GpuBuffer,
    extract::{RenderResource, RenderResourcePlugin},
    graph::Slot,
    prelude::*,
    WgpuDevice, WgpuQueue,
};
use weaver_util::prelude::Result;

#[derive(Copy, Clone, Debug, Component, Reflect)]
pub struct PointLight {
    pub color: Color,
    pub intensity: f32,
    pub radius: f32,
}

#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PointLightUniform {
    pub position: Vec3,
    _padding: u32,
    pub color: Color,
    pub intensity: f32,
    pub radius: f32,
    _padding2: [u32; 2],
}

pub const MAX_POINT_LIGHTS: usize = 16;

#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PointLightArrayUniform {
    pub count: u32,
    _padding: [u32; 3],
    pub point_lights: [PointLightUniform; MAX_POINT_LIGHTS],
}

impl Default for PointLightArrayUniform {
    fn default() -> Self {
        Self {
            count: 0,
            _padding: [0; 3],
            point_lights: [PointLightUniform::default(); MAX_POINT_LIGHTS],
        }
    }
}

impl From<Vec<PointLightUniform>> for PointLightArrayUniform {
    fn from(point_lights: Vec<PointLightUniform>) -> Self {
        let mut point_light_array = PointLightArrayUniform {
            count: point_lights.len() as u32,
            ..Default::default()
        };
        point_light_array.point_lights[..point_lights.len()].copy_from_slice(&point_lights);
        point_light_array
    }
}

#[derive(Resource)]
pub struct GpuPointLightArray {
    pub buffer: GpuBuffer,
}

impl RenderResource for GpuPointLightArray {
    fn extract_render_resource(main_world: &mut World, render_world: &mut World) -> Option<Self>
    where
        Self: Sized,
    {
        let point_lights = main_world.query::<(&PointLight, &Transform)>();

        let point_light_uniforms: Vec<PointLightUniform> = point_lights
            .iter()
            .map(|(_, (point_light, transform))| PointLightUniform {
                position: transform.translation,
                color: point_light.color,
                intensity: point_light.intensity,
                radius: point_light.radius,
                _padding: 0,
                _padding2: [0; 2],
            })
            .collect();

        let device = render_world.get_resource::<WgpuDevice>().unwrap();

        let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Point Light Storage Buffer"),
            contents: bytemuck::cast_slice(&[PointLightArrayUniform::from(point_light_uniforms)]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        Some(Self {
            buffer: GpuBuffer::new(storage_buffer),
        })
    }

    fn update_render_resource(
        &mut self,
        main_world: &mut World,
        render_world: &mut World,
    ) -> Result<()> {
        let point_lights = main_world.query::<(&PointLight, &Transform)>();

        let point_light_uniforms: Vec<PointLightUniform> = point_lights
            .iter()
            .map(|(_, (point_light, transform))| PointLightUniform {
                position: transform.translation,
                color: point_light.color,
                intensity: point_light.intensity,
                radius: point_light.radius,
                _padding: 0,
                _padding2: [0; 2],
            })
            .collect();

        let queue = render_world.get_resource::<WgpuQueue>().unwrap();

        self.buffer.update(
            &queue,
            bytemuck::cast_slice(&[PointLightArrayUniform::from(point_light_uniforms)]),
        );

        Ok(())
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
                resource: wgpu::BindingResource::Buffer(self.buffer.as_entire_buffer_binding()),
            }],
            label: Some("Point Light Bind Group"),
        })
    }
}

pub struct PointLightPlugin;

impl Plugin for PointLightPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_plugin(RenderResourcePlugin::<GpuPointLightArray>::default())?;
        render_app.add_plugin(ResourceBindGroupPlugin::<GpuPointLightArray>::default())?;

        Ok(())
    }
}

pub struct PointLightArrayNode;

impl Render for PointLightArrayNode {
    fn render(&self, render_world: &mut World, _input_slots: &[Slot]) -> Result<Vec<Slot>> {
        let bind_group = render_world
            .get_resource::<BindGroup<GpuPointLightArray>>()
            .expect("Point Light Array Bind Group resource not present");

        Ok(vec![Slot::BindGroup(bind_group.bind_group().clone())])
    }
}
