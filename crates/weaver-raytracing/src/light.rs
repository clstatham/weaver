use encase::ShaderType;
use weaver_app::{plugin::Plugin, App};
use weaver_core::{
    color::Color,
    prelude::{Transform, Vec3},
};
use weaver_ecs::{
    prelude::{Commands, Res, ResMut},
    query::{Query, QueryableItem},
    world::{ConstructFromWorld, World},
};
use weaver_renderer::{
    extract::{Extract, ExtractComponentPlugin},
    prelude::*,
};

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct PointLight {
    pub color: Color,
    pub intensity: f32,
}

impl ExtractComponent for PointLight {
    type ExtractQueryFetch = &'static Self;
    type Out = Self;

    fn extract_render_component(item: QueryableItem<Self::ExtractQueryFetch>) -> Option<Self::Out> {
        Some(*item)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, ShaderType)]
#[repr(C)]
pub struct PointLightUniform {
    pub position: Vec3,
    _padding: u32,
    pub color: Color,
    pub intensity: f32,
}

impl From<PointLight> for PointLightUniform {
    fn from(light: PointLight) -> Self {
        Self {
            position: Vec3::ZERO,
            _padding: 0,
            color: light.color,
            intensity: light.intensity,
        }
    }
}

impl From<(PointLight, Transform)> for PointLightUniform {
    fn from((light, transform): (PointLight, Transform)) -> Self {
        Self {
            position: transform.translation,
            _padding: 0,
            color: light.color,
            intensity: light.intensity,
        }
    }
}

pub struct RaytracingLightingInformation {
    pub point_lights: GpuBufferVec<PointLightUniform>,
}

impl ConstructFromWorld for RaytracingLightingInformation {
    fn from_world(world: &World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();
        let mut buffer =
            GpuBufferVec::new(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        buffer.reserve(1, &device);
        Self {
            point_lights: buffer,
        }
    }
}

impl CreateBindGroup for RaytracingLightingInformation {
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
            label: Some("Raytracing Lighting Information Bind Group Layout"),
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
                resource: self.point_lights.binding().unwrap(),
            }],
            label: Some("Raytracing Lighting Information Bind Group"),
        })
    }
}

pub struct PointLightPlugin;

impl Plugin for PointLightPlugin {
    fn build(&self, render_app: &mut App) -> weaver_util::prelude::Result<()> {
        render_app.add_plugin(ExtractComponentPlugin::<PointLight>::default())?;
        Ok(())
    }
}

pub async fn init_raytracing_lighting_information(commands: Commands) {
    if !commands
        .has_resource::<RaytracingLightingInformation>()
        .await
    {
        commands
            .init_resource::<RaytracingLightingInformation>()
            .await;
    }
}

pub async fn update_raytracing_lighting_information(
    mut lighting: ResMut<RaytracingLightingInformation>,
    mut lights: Extract<Query<(&PointLight, Option<&Transform>)>>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) {
    lighting.point_lights.clear();
    for (light, transform) in lights.iter() {
        if let Some(transform) = transform {
            let uniform = (*light, *transform).into();
            lighting.point_lights.push(uniform);
        } else {
            let uniform = (*light).into();
            lighting.point_lights.push(uniform);
        }
    }

    lighting.point_lights.enqueue_update(&device, &queue);
}
