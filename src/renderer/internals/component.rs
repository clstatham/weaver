use std::sync::Arc;

use crate::ecs::{Component, World};

use super::{BindGroupLayoutCache, GpuHandle, GpuResourceManager};

/// A component that holds handles to GPU resources.
pub trait GpuComponent: Component {
    /// Lazily initialize the component's GPU resources, or return the existing handles if they have already been initialized.
    fn lazy_init(&self, manager: &GpuResourceManager) -> anyhow::Result<Vec<GpuHandle>>;

    /// Updates the component's GPU resources with new data.
    fn update_resources(&self, world: &World) -> anyhow::Result<()>;
    /// Destroys the component's GPU resources.
    fn destroy_resources(&self) -> anyhow::Result<()>;
}

/// A component that holds a GPU bind group.
pub trait BindableComponent: Component {
    /// Creates a bind group layout for the component.
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout;

    /// Creates a bind group for the component.
    fn create_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>>;

    /// Returns the bind group for the component, if it has been created.
    fn bind_group(&self) -> Option<Arc<wgpu::BindGroup>>;

    /// Lazily initializes the bind group for the component, or returns the existing bind group if it has already been initialized.
    fn lazy_init_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>>;
}
