use std::sync::Arc;

use weaver_ecs::{StaticId, World};

use super::{BindGroupLayoutCache, GpuResourceManager};

/// A component that holds handles to GPU resources.
pub trait GpuComponent: StaticId {
    /// Lazily initialize the component's GPU resources, or return the existing handles if they have already been initialized.
    fn lazy_init(&self, manager: &GpuResourceManager) -> anyhow::Result<()>;

    /// Updates the component's GPU resources with new data.
    fn update_resources(&self, world: &World) -> anyhow::Result<()>;
    /// Destroys the component's GPU resources.
    fn destroy_resources(&self) -> anyhow::Result<()>;
}

/// A component that holds a GPU bind group.
pub trait BindableComponent: StaticId {
    /// Creates a bind group layout for the component.
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized;

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
