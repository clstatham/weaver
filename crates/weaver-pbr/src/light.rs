use weaver_app::{plugin::Plugin, App};
use wgpu::util::DeviceExt;

use weaver_core::{color::Color, prelude::Vec3};
use weaver_ecs::prelude::{Component, Query, World};
use weaver_renderer::{
    bind_group::{BindGroup, CreateBindGroup, ResourceBindGroupPlugin},
    buffer::GpuBuffer,
    extract::{RenderResource, RenderResourcePlugin},
    graph::Slot,
    prelude::*,
};
use weaver_util::prelude::Result;

#[derive(Copy, Clone, Debug, Component)]
pub struct PointLight {
    pub position: Vec3,
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

impl From<PointLight> for PointLightUniform {
    fn from(point_light: PointLight) -> Self {
        Self {
            position: point_light.position,
            _padding: 0,
            color: point_light.color,
            intensity: point_light.intensity,
            radius: point_light.radius,
            _padding2: [0; 2],
        }
    }
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

#[derive(Component)]
pub struct GpuPointLightArray {
    pub buffer: GpuBuffer,
}

impl RenderResource for GpuPointLightArray {
    fn extract_render_resource(world: &World, renderer: &Renderer) -> Option<Self>
    where
        Self: Sized,
    {
        let point_lights = world.query(&Query::new().read::<PointLight>());

        let point_light_uniforms: Vec<PointLightUniform> = point_lights
            .iter()
            .map(|entity| {
                let point_light = world.get_component::<PointLight>(entity).unwrap();
                PointLightUniform::from(*point_light)
            })
            .collect();

        let storage_buffer =
            renderer
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Point Light Storage Buffer"),
                    contents: bytemuck::cast_slice(&[PointLightArrayUniform::from(
                        point_light_uniforms,
                    )]),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                });

        Some(Self {
            buffer: GpuBuffer::new(storage_buffer),
        })
    }

    fn update_render_resource(&mut self, world: &World, renderer: &Renderer) -> Result<()> {
        let point_lights = world.query(&Query::new().read::<PointLight>());

        let point_light_uniforms: Vec<PointLightUniform> = point_lights
            .iter()
            .map(|entity| {
                let point_light = world.get_component::<PointLight>(entity).unwrap();
                PointLightUniform::from(*point_light)
            })
            .collect();

        self.buffer.update(
            renderer.queue(),
            bytemuck::cast_slice(&[PointLightArrayUniform::from(point_light_uniforms)]),
        );

        Ok(())
    }
}

impl CreateBindGroup for GpuPointLightArray {
    fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
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

    fn create_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &Self::bind_group_layout(device),
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
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(RenderResourcePlugin::<GpuPointLightArray>::default())?;
        app.add_plugin(ResourceBindGroupPlugin::<GpuPointLightArray>::default())?;

        Ok(())
    }
}

pub struct PointLightArrayNode;

impl Render for PointLightArrayNode {
    fn render(
        &self,
        world: &World,
        _renderer: &Renderer,
        _input_slots: &[Slot],
    ) -> Result<Vec<Slot>> {
        let bind_group = world
            .get_resource::<BindGroup<GpuPointLightArray>>()
            .expect("Point Light Array Bind Group resource not present");

        Ok(vec![Slot::BindGroup(bind_group.bind_group().clone())])
    }
}
