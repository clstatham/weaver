pub trait RenderResource {
    fn create(device: &wgpu::Device, queue: &wgpu::Queue) -> Self;
    fn update(&self, device: &wgpu::Device, queue: &wgpu::Queue);
    fn destroy(&self);
}

pub trait AsRenderResource {
    type Resource: RenderResource;

    fn as_render_resource(&self) -> &Self::Resource;
    fn as_render_resource_mut(&mut self) -> &mut Self::Resource;
}
