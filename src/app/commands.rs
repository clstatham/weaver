use std::cell::RefCell;

use weaver_ecs::{Bundle, Resource, World};

use crate::{core::texture::Texture, renderer::Renderer};

pub struct Commands<'a> {
    world: RefCell<&'a mut World>,
    renderer: RefCell<&'a mut Renderer>,
}

impl<'a> Commands<'a> {
    pub fn new(world: &'a mut World, renderer: &'a mut Renderer) -> Self {
        Self {
            world: RefCell::new(world),
            renderer: RefCell::new(renderer),
        }
    }

    pub fn spawn<T: Bundle>(&self, bundle: T) {
        self.world.borrow_mut().spawn(bundle);
    }

    pub fn insert_resource<T: Resource>(&self, resource: T) {
        self.world.borrow_mut().insert_resource(resource);
    }

    pub fn load_cubemap(&self, path: &str, dst_size: u32) -> Texture {
        let renderer = self.renderer.borrow();
        renderer
            .hdr_loader
            .load(&renderer.device, &renderer.queue, dst_size, path)
            .unwrap()
    }
}
