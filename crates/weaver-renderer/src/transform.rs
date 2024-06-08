use weaver_app::{plugin::Plugin, App};
use wgpu::util::DeviceExt;

use weaver_core::transform::Transform;
use weaver_ecs::{entity::Entity, query::Query, world::World};

use crate::{
    bind_group::{BindGroupPlugin, CreateBindGroup},
    buffer::GpuBuffer,
    extract::{RenderComponent, RenderComponentPlugin},
    Renderer,
};

pub struct GpuTransform {
    pub buffer: GpuBuffer,
}

impl RenderComponent for GpuTransform {
    fn extract_query() -> Query {
        Query::new().read::<Transform>()
    }

    fn extract_render_component(entity: Entity, world: &World, renderer: &Renderer) -> Option<Self>
    where
        Self: Sized,
    {
        let transform = world.get_component::<Transform>(entity)?;

        let buffer = GpuBuffer::new(renderer.device().create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Transform Buffer"),
                contents: bytemuck::cast_slice(&[transform.matrix()]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            },
        ));

        Some(Self { buffer })
    }

    fn update_render_component(
        &mut self,
        entity: Entity,
        world: &World,
        renderer: &Renderer,
    ) -> anyhow::Result<()> {
        let Some(transform) = world.get_component::<Transform>(entity) else {
            return Ok(());
        };

        self.buffer.update(
            renderer.queue(),
            bytemuck::cast_slice(&[transform.matrix()]),
        );

        Ok(())
    }
}

impl CreateBindGroup for GpuTransform {
    fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Transform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    fn create_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &Self::bind_group_layout(device),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &self.buffer,
                    offset: 0,
                    size: None,
                }),
            }],
            label: Some("Transform Bind Group"),
        })
    }
}

pub struct TransformPlugin;

impl Plugin for TransformPlugin {
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        app.add_plugin(RenderComponentPlugin::<GpuTransform>::default())?;
        app.add_plugin(BindGroupPlugin::<GpuTransform>::default())?;
        Ok(())
    }
}
