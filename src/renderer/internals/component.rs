use std::sync::Arc;

use crate::ecs::{Component, World};

use super::{BindGroupLayoutCache, GpuHandle, GpuResourceManager};

pub trait GpuComponent: Component {
    fn lazy_init(&self, manager: &GpuResourceManager) -> anyhow::Result<Vec<GpuHandle>>;
    fn update_resources(&self, world: &World) -> anyhow::Result<()>;
    fn destroy_resources(&self) -> anyhow::Result<()>;
}

pub trait BindableComponent: Component {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout;
    fn create_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>>;
    fn bind_group(&self) -> Option<Arc<wgpu::BindGroup>>;
    fn lazy_init_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>>;
}
