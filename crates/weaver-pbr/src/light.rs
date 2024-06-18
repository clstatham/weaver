use encase::ShaderType;
use weaver_app::{plugin::Plugin, App};

use weaver_core::{color::Color, prelude::Vec3, transform::Transform};
use weaver_ecs::prelude::{Component, Reflect, Resource, World};
use weaver_renderer::{
    bind_group::{BindGroup, BindGroupLayout, CreateBindGroup, ResourceBindGroupPlugin},
    buffer::GpuBufferVec,
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

impl RenderResource for GpuPointLightArray {
    fn extract_render_resource(_main_world: &mut World, _render_world: &mut World) -> Option<Self>
    where
        Self: Sized,
    {
        let buffer = GpuBufferVec::new(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        Some(Self { buffer })
    }

    fn update_render_resource(
        &mut self,
        main_world: &mut World,
        render_world: &mut World,
    ) -> Result<()> {
        let point_lights = main_world.query::<(&PointLight, &Transform)>();

        let device = render_world.get_resource::<WgpuDevice>().unwrap();
        let queue = render_world.get_resource::<WgpuQueue>().unwrap();

        self.buffer.clear();
        for (_entity, (point_light, transform)) in point_lights.iter() {
            let uniform = PointLightUniform {
                position: transform.translation,
                color: point_light.color,
                intensity: point_light.intensity,
                radius: point_light.radius,
                _padding: 0,
            };
            self.buffer.push(uniform);
        }

        self.buffer.enqueue_update(&device, &queue);

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
                resource: self.buffer.binding().unwrap(),
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
    fn render(&mut self, render_world: &mut World, _input_slots: &[Slot]) -> Result<Vec<Slot>> {
        let bind_group = render_world
            .get_resource::<BindGroup<GpuPointLightArray>>()
            .expect("Point Light Array Bind Group resource not present");

        Ok(vec![Slot::BindGroup(bind_group.bind_group().clone())])
    }
}
