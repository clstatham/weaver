use crate::{
    core::texture::Texture,
    ecs::{Bundle, World},
    renderer::Renderer,
};

pub struct Commands<'a> {
    world: &'a mut World,
}

impl<'a> Commands<'a> {
    pub fn new(world: &'a mut World) -> Self {
        Self { world }
    }

    pub fn spawn<T: Bundle>(&mut self, bundle: T) {
        bundle.build(self.world);
    }

    pub fn load_cubemap(&self, path: &str, dst_size: u32) -> Texture {
        let renderer = self.world.read_resource::<Renderer>();
        renderer
            .hdr_loader
            .load(&renderer.device, &renderer.queue, dst_size, path)
            .unwrap()
    }
}
