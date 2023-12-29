use std::cell::RefCell;

use weaver_ecs::{Bundle, World};

use crate::{core::mesh::Mesh, renderer::Renderer};

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

    pub fn load_gltf(&self, path: &str) -> anyhow::Result<Mesh> {
        let renderer = self.renderer.borrow();
        Mesh::load_gltf(
            path,
            &renderer.device,
            &renderer.queue,
            &renderer.model_pass.model_buffer,
            &renderer.model_pass.view_buffer,
            &renderer.model_pass.proj_buffer,
        )
    }
}
