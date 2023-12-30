use std::cell::RefCell;

use weaver_ecs::{Bundle, World};

use crate::{
    core::{model::Model, texture::Texture},
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

    pub fn load_gltf(&self, path: &str, use_texture: bool) -> Model {
        let renderer = self.renderer.borrow();
        Model::load_gltf(path, &renderer, use_texture).unwrap()
    }

    pub fn load_texture(&self, path: &str, is_normal_map: bool) -> Texture {
        let renderer = self.renderer.borrow();
        Texture::load(path, &renderer.device, &renderer.queue, None, is_normal_map)
    }
}
