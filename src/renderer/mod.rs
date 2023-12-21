use std::time::Duration;

use crate::{
    core::{camera::PerspectiveCamera, color::Color, mesh::Mesh, transform::Transform},
    ecs::world::World,
};
#[macro_use]
pub mod shader;
mod render_impl;

pub struct Renderer {
    screen_width: usize,
    screen_height: usize,
    color_buffer: Vec<Color>,
    depth_buffer: Vec<f32>,
    camera: PerspectiveCamera,
}

impl Renderer {
    pub fn new(screen_width: usize, screen_height: usize) -> Self {
        let mut camera = PerspectiveCamera::new(
            glam::Vec3::new(1.0, 1.0, 1.0),
            glam::Vec3::ZERO,
            120.0f32.to_radians(),
            screen_width as f32 / screen_height as f32,
            0.001,
            100000.0,
        );
        camera.look_at(
            glam::Vec3::new(4.0, 4.0, 4.0),
            glam::Vec3::new(0.0, 0.0, 0.0),
            glam::Vec3::NEG_Y,
        );
        Self {
            screen_width,
            screen_height,
            color_buffer: vec![Color::new(0.0, 0.0, 0.0); screen_width * screen_height],
            depth_buffer: vec![f32::INFINITY; screen_width * screen_height],
            camera,
        }
    }

    pub fn color_buffer(&self) -> &[Color] {
        &self.color_buffer
    }

    pub fn clear(&mut self, color: Color) {
        for pixel in self.color_buffer.iter_mut() {
            *pixel = color;
        }

        for pixel in self.depth_buffer.iter_mut() {
            *pixel = f32::INFINITY;
        }
    }

    pub fn set_color(&mut self, x: usize, y: usize, color: Color) {
        if x >= self.screen_width || y >= self.screen_height {
            return;
        }
        let index = y * self.screen_width + x;
        self.color_buffer[index] = color;
    }

    pub fn get_color(&self, x: usize, y: usize) -> Color {
        let index = y * self.screen_width + x;
        self.color_buffer[index]
    }

    pub fn set_depth(&mut self, x: usize, y: usize, depth: f32) {
        if x >= self.screen_width || y >= self.screen_height {
            return;
        }
        let index = y * self.screen_width + x;
        self.depth_buffer[index] = depth;
    }

    pub fn get_depth(&self, x: usize, y: usize) -> f32 {
        let index = y * self.screen_width + x;
        self.depth_buffer[index]
    }

    pub fn camera(&self) -> &PerspectiveCamera {
        &self.camera
    }

    pub fn screen_width(&self) -> usize {
        self.screen_width
    }

    pub fn screen_height(&self) -> usize {
        self.screen_height
    }

    pub fn view_to_screen(&self, (x, y): (f32, f32)) -> (usize, usize) {
        let x = (x + 1.0) / 2.0 * self.screen_width as f32;
        let y = (y + 1.0) / 2.0 * self.screen_height as f32;
        (x as usize, y as usize)
    }

    pub fn screen_to_view(&self, (x, y): (usize, usize)) -> (f32, f32) {
        let x = x as f32 / self.screen_width as f32 * 2.0 - 1.0;
        let y = y as f32 / self.screen_height as f32 * 2.0 - 1.0;
        (x, y)
    }

    pub fn render(&mut self, world: &mut World, _delta: Duration) {
        self.clear(Color::new(0.0, 0.0, 0.0));

        for (mesh, transform) in world.read::<(Mesh, Transform)>() {
            for i in (0..mesh.indices.len()).step_by(3) {
                let i0 = mesh.indices[i] as usize;
                let i1 = mesh.indices[i + 1] as usize;
                let i2 = mesh.indices[i + 2] as usize;

                let v0 = mesh.vertices[i0];
                let v1 = mesh.vertices[i1];
                let v2 = mesh.vertices[i2];
                self.wireframe_triangle(
                    v0,
                    v1,
                    v2,
                    &vertex_shader!(shader::TransformVertexShader {
                        transform: *transform,
                    }),
                    &fragment_shader!(shader::VertexColorFragmentShader),
                );
            }
        }
    }
}

