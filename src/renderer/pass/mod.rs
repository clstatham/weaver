use std::io::Read;

use crate::{core::texture::Texture, ecs::World};

pub mod doodads;
pub mod hdr;
pub mod particles;
pub mod pbr;
pub mod shadow;
pub mod sky;

pub fn preprocess_shader(shader: &str) -> String {
    let mut output = String::new();

    // find all #import directives
    let lines = shader.lines();
    for line in lines {
        if line.starts_with("//#import") {
            let mut path = line.split('"');
            path.next();
            let path = path.next().unwrap();
            let path = std::path::Path::new(path);

            let mut file = std::fs::File::open(path).unwrap();
            let mut file_contents = String::new();
            file.read_to_string(&mut file_contents).unwrap();

            output.push_str(&preprocess_shader(&file_contents));
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }

    output
}

#[macro_export]
macro_rules! include_shader {
    ($name:literal) => {
        $crate::renderer::pass::preprocess_shader(include_str!($name))
    };
}

pub trait Pass {
    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_target: &Texture,
        depth_target: &Texture,
        world: &World,
    ) -> anyhow::Result<()>;
}
