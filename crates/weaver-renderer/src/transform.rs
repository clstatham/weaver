use encase::ShaderType;
use weaver_app::{App, plugin::Plugin};
use weaver_ecs::{
    entity::Entity,
    prelude::{Commands, Res},
    query::{Query, QueryableItem},
    system::IntoSystemConfig,
};
use weaver_util::prelude::*;

use weaver_core::transform::Transform;

use crate::{
    RenderStage, WgpuDevice, WgpuQueue,
    buffer::{GpuBuffer, GpuBufferVec},
    extract::{ExtractComponent, ExtractComponentPlugin},
    prelude::{
        BindGroupLayout, ComponentBindGroupPlugin, CreateBindGroup, create_component_bind_group,
    },
};

impl ExtractComponent for Transform {
    type ExtractQueryFetch = &'static Transform;
    type Out = GpuTransform;

    fn extract_render_component(
        item: QueryableItem<'_, Self::ExtractQueryFetch>,
    ) -> Option<Self::Out> {
        let matrix = item.matrix();

        Some(GpuTransform { matrix })
    }
}

#[derive(Copy, Clone, Debug, ShaderType)]
#[repr(C)]
pub struct GpuTransform {
    matrix: glam::Mat4,
}

pub struct TransformBindGroup {
    buffer: GpuBufferVec<GpuTransform>,
}

impl CreateBindGroup for TransformBindGroup {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
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

    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        cached_layout: &BindGroupLayout,
    ) -> wgpu::BindGroup
    where
        Self: Sized,
    {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Transform Bind Group"),
            layout: cached_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.buffer.binding().unwrap(),
            }],
        })
    }
}

async fn extract_transform_bind_groups(
    commands: Commands,
    mut query: Query<(Entity, &GpuTransform, Option<&mut TransformBindGroup>)>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) {
    let mut to_insert = Vec::new();
    for (entity, transform, mut bind_group) in query.iter() {
        if let Some(bind_group) = bind_group.as_mut() {
            bind_group.buffer.clear();
            bind_group.buffer.push(*transform);
            bind_group.buffer.enqueue_update(&device, &queue);
        } else {
            let mut buffer =
                GpuBufferVec::new(wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
            buffer.push(*transform);
            buffer.enqueue_update(&device, &queue);

            let bind_group = TransformBindGroup { buffer };
            to_insert.push((entity, bind_group));
        }
    }

    drop(query);

    for (entity, bind_group) in to_insert {
        commands.insert_component(entity, bind_group);
    }
}

pub struct TransformPlugin;

impl Plugin for TransformPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(ExtractComponentPlugin::<Transform>::default())?;
        app.add_plugin(ComponentBindGroupPlugin::<TransformBindGroup>::default())?;

        app.add_system(
            extract_transform_bind_groups.after(create_component_bind_group::<TransformBindGroup>),
            RenderStage::ExtractBindGroup,
        );
        Ok(())
    }
}
