//! A render pass wrapper that skips redundant state changes.

/// Wraps a [`wgpu::RenderPass`] and remembers the bound pipeline, bind groups and buffers, so
/// setting the same one again (common when consecutive draws share a material or mesh) is free.
///
/// Bound objects are identified by address. That's sound because everything bound in a pass is
/// owned by the render world, which doesn't drop anything while the pass is being recorded.
pub struct TrackedRenderPass<'a> {
    pass: wgpu::RenderPass<'a>,
    pipeline: Option<*const wgpu::RenderPipeline>,
    bind_groups: [Option<*const wgpu::BindGroup>; 8],
    vertex_buffers: [Option<*const wgpu::Buffer>; 8],
    index_buffer: Option<*const wgpu::Buffer>,
}

impl<'a> TrackedRenderPass<'a> {
    pub fn new(pass: wgpu::RenderPass<'a>) -> Self {
        Self {
            pass,
            pipeline: None,
            bind_groups: [None; 8],
            vertex_buffers: [None; 8],
            index_buffer: None,
        }
    }

    pub fn set_pipeline(&mut self, pipeline: &wgpu::RenderPipeline) {
        let ptr = pipeline as *const _;
        if self.pipeline != Some(ptr) {
            self.pass.set_pipeline(pipeline);
            self.pipeline = Some(ptr);
        }
    }

    pub fn set_bind_group(&mut self, index: u32, bind_group: &wgpu::BindGroup) {
        let ptr = bind_group as *const _;
        let slot = &mut self.bind_groups[index as usize];
        if *slot != Some(ptr) {
            self.pass.set_bind_group(index, bind_group, &[]);
            *slot = Some(ptr);
        }
    }

    /// Binds the whole buffer.
    pub fn set_vertex_buffer(&mut self, slot: u32, buffer: &wgpu::Buffer) {
        let ptr = buffer as *const _;
        let tracked = &mut self.vertex_buffers[slot as usize];
        if *tracked != Some(ptr) {
            self.pass.set_vertex_buffer(slot, buffer.slice(..));
            *tracked = Some(ptr);
        }
    }

    /// Binds the whole buffer.
    pub fn set_index_buffer(&mut self, buffer: &wgpu::Buffer, format: wgpu::IndexFormat) {
        let ptr = buffer as *const _;
        if self.index_buffer != Some(ptr) {
            self.pass.set_index_buffer(buffer.slice(..), format);
            self.index_buffer = Some(ptr);
        }
    }

    pub fn draw(&mut self, vertices: std::ops::Range<u32>, instances: std::ops::Range<u32>) {
        self.pass.draw(vertices, instances);
    }

    pub fn draw_indexed(
        &mut self,
        indices: std::ops::Range<u32>,
        base_vertex: i32,
        instances: std::ops::Range<u32>,
    ) {
        self.pass.draw_indexed(indices, base_vertex, instances);
    }

    /// The underlying pass, for anything not covered here. State set through it isn't tracked.
    pub fn raw(&mut self) -> &mut wgpu::RenderPass<'a> {
        self.pipeline = None;
        self.bind_groups = [None; 8];
        self.vertex_buffers = [None; 8];
        self.index_buffer = None;
        &mut self.pass
    }
}
