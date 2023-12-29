use std::cell::RefCell;

use weaver_ecs::{Bundle, Entity, World};

use crate::{
    core::{mesh::Mesh, model::Model},
    renderer::Renderer,
};

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

    pub fn load_gltf(&self, path: &str) -> Model {
        let renderer = self.renderer.borrow();
        Model::load_gltf(path, &renderer).unwrap()
    }
}
