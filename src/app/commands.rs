use crate::{
    core::texture::Texture,
    ecs::{
        entity::Entity,
        resource::{Res, Resource},
        Bundle, World,
    },
    renderer::Renderer,
};

pub struct Commands<'a> {
    world: &'a World,
}

impl<'a> Commands<'a> {
    pub fn new(world: &'a World) -> Self {
        Self { world }
    }

    pub fn spawn<T: Bundle>(&self, bundle: T) -> anyhow::Result<Entity> {
        bundle.build(self.world)
    }

    pub fn read_resource<T: Resource>(&self) -> anyhow::Result<Res<T>> {
        self.world.read_resource()
    }

    pub fn load_cubemap(&self, path: &str, dst_size: u32) -> anyhow::Result<Texture> {
        let renderer = self.world.read_resource::<Renderer>()?;
        renderer
            .hdr_loader
            .load(&renderer.device, &renderer.queue, dst_size, path)
    }
}
