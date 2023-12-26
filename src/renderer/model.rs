use crate::core::{camera::Camera, model::Model};

pub trait DrawModel<'a> {
    fn draw_model(&mut self, model: &'a Model, camera: &'a Camera);
}

impl<'a, 'b: 'a> DrawModel<'b> for wgpu::RenderPass<'a> {
    fn draw_model(&mut self, model: &'b Model, camera: &'b Camera) {
        // self.set_bind_group(0, &model.mesh.bind_group, &[]);
        self.set_vertex_buffer(0, model.mesh.vertex_buffer.slice(..));
        self.set_index_buffer(model.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        // self.set_bind_group(1, &camera.bind_group, &[]);
        self.draw_indexed(0..model.mesh.num_indices, 0, 0..1);
    }
}
