use pixels::Pixels;
use rayon::prelude::*;

use crate::{
    core::{
        camera::PerspectiveCamera, color::Color, light::Light, mesh::Mesh, transform::Transform,
    },
    ecs::world::World,
};

use self::shader::FragmentShader;

use super::Renderer;

pub mod render_impl;
#[macro_use]
pub mod shader;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Pixel {
    pub x: i32,
    pub y: i32,
    pub color: Color,
    pub depth: f32,
    pub normal: glam::Vec3,
    pub position: glam::Vec3,
}

pub struct SoftwareRenderer {
    pixels: Pixels,

    screen_width: usize,
    screen_height: usize,
    color_buffer: Vec<Color>,
    depth_buffer: Vec<f32>,
    normal_buffer: Vec<glam::Vec3>,
    position_buffer: Vec<glam::Vec3>,

    pixel_cache: Vec<Pixel>,
}

impl SoftwareRenderer {
    pub fn new(window: &winit::window::Window) -> Self {
        let screen_width = window.inner_size().width as usize;
        let screen_height = window.inner_size().height as usize;

        let pixels = {
            let window_size = window.inner_size();
            let surface_texture =
                pixels::SurfaceTexture::new(window_size.width, window_size.height, window);
            Pixels::new(screen_width as u32, screen_height as u32, surface_texture).unwrap()
        };

        Self {
            pixels,
            screen_width,
            screen_height,
            color_buffer: vec![Color::new(0.0, 0.0, 0.0); screen_width * screen_height],
            depth_buffer: vec![f32::INFINITY; screen_width * screen_height],
            normal_buffer: vec![glam::Vec3::ZERO; screen_width * screen_height],
            position_buffer: vec![glam::Vec3::ZERO; screen_width * screen_height],
            pixel_cache: Vec::with_capacity(screen_width * screen_height),
        }
    }

    #[inline]
    pub fn color_buffer(&self) -> &[Color] {
        &self.color_buffer
    }

    #[inline]
    pub fn clear(&mut self, color: Color) {
        self.color_buffer.fill(color);
        self.depth_buffer.fill(f32::INFINITY);
        self.normal_buffer.fill(glam::Vec3::ZERO);
        self.position_buffer.fill(glam::Vec3::ZERO);
        self.pixel_cache.clear();
    }

    #[inline]
    pub fn set_color(&mut self, x: usize, y: usize, color: Color) {
        if x >= self.screen_width || y >= self.screen_height {
            return;
        }
        let index = y * self.screen_width + x;
        self.color_buffer[index] = color;
    }

    #[inline]
    pub fn get_color(&self, x: usize, y: usize) -> Color {
        if x >= self.screen_width || y >= self.screen_height {
            return Color::BLACK;
        }
        let index = y * self.screen_width + x;
        self.color_buffer[index]
    }

    #[inline]
    pub fn set_depth(&mut self, x: usize, y: usize, depth: f32) {
        if x >= self.screen_width || y >= self.screen_height {
            return;
        }
        let index = y * self.screen_width + x;
        self.depth_buffer[index] = depth;
    }

    #[inline]
    pub fn get_depth(&self, x: usize, y: usize) -> f32 {
        if x >= self.screen_width || y >= self.screen_height {
            return f32::INFINITY;
        }
        let index = y * self.screen_width + x;
        self.depth_buffer[index]
    }

    #[inline]
    pub fn set_normal(&mut self, x: usize, y: usize, normal: glam::Vec3) {
        if x >= self.screen_width || y >= self.screen_height {
            return;
        }
        let index = y * self.screen_width + x;
        self.normal_buffer[index] = normal;
    }

    #[inline]
    pub fn get_normal(&self, x: usize, y: usize) -> glam::Vec3 {
        if x >= self.screen_width || y >= self.screen_height {
            return glam::Vec3::ZERO;
        }
        let index = y * self.screen_width + x;
        self.normal_buffer[index]
    }

    #[inline]
    pub fn set_position(&mut self, x: usize, y: usize, position: glam::Vec3) {
        if x >= self.screen_width || y >= self.screen_height {
            return;
        }
        let index = y * self.screen_width + x;
        self.position_buffer[index] = position;
    }

    #[inline]
    pub fn get_position(&self, x: usize, y: usize) -> glam::Vec3 {
        if x >= self.screen_width || y >= self.screen_height {
            return glam::Vec3::ZERO;
        }
        let index = y * self.screen_width + x;
        self.position_buffer[index]
    }

    #[inline]
    pub fn screen_width(&self) -> usize {
        self.screen_width
    }

    #[inline]
    pub fn screen_height(&self) -> usize {
        self.screen_height
    }

    #[inline]
    pub fn view_to_screen(&self, (x, y): (f32, f32)) -> (i32, i32) {
        let x = (x + 1.0) / 2.0 * self.screen_width as f32;
        let y = (y + 1.0) / 2.0 * self.screen_height as f32;
        (x as i32, y as i32)
    }

    #[inline]
    pub fn screen_to_view(&self, (x, y): (usize, usize)) -> (f32, f32) {
        let x = x as f32 / self.screen_width as f32 * 2.0 - 1.0;
        let y = y as f32 / self.screen_height as f32 * 2.0 - 1.0;
        (x, y)
    }

    fn render_impl(&mut self, world: &mut World) {
        self.clear(Color::new(0.1, 0.1, 0.1));

        // query the world for entities that have both a mesh and transform
        let query = world.read::<(Mesh, Transform)>();

        let camera = world.read_resource::<PerspectiveCamera>().unwrap();

        // rasterize each mesh
        for (mesh, transform) in query
            .get::<Mesh>()
            .into_iter()
            .zip(query.get::<Transform>())
        {
            let pixels = (0..mesh.indices.len())
                .step_by(3)
                .flat_map(|i| {
                    let i0 = mesh.indices[i] as usize;
                    let i1 = mesh.indices[i + 1] as usize;
                    let i2 = mesh.indices[i + 2] as usize;

                    let v0 = mesh.vertices[i0];
                    let v1 = mesh.vertices[i1];
                    let v2 = mesh.vertices[i2];

                    self.triangle(
                        v0,
                        v1,
                        v2,
                        &vertex_shader!(shader::TransformVertexShader {
                            transform: *transform,
                        }),
                        mesh.texture.as_ref(),
                        &camera,
                    )
                })
                .collect::<Vec<_>>();
            self.pixel_cache.extend(pixels);
        }

        for pixel in self.pixel_cache.drain(..) {
            // check if pixels is outside screen
            if pixel.x < 0
                || pixel.y < 0
                || pixel.x >= self.screen_width as i32
                || pixel.y >= self.screen_height as i32
            {
                continue;
            }
            // check depth test
            if self.depth_buffer[pixel.y as usize * self.screen_width + pixel.x as usize]
                < pixel.depth
            {
                continue;
            }
            self.color_buffer[pixel.y as usize * self.screen_width + pixel.x as usize] =
                pixel.color;
            self.depth_buffer[pixel.y as usize * self.screen_width + pixel.x as usize] =
                pixel.depth;
            self.normal_buffer[pixel.y as usize * self.screen_width + pixel.x as usize] =
                pixel.normal;
            self.position_buffer[pixel.y as usize * self.screen_width + pixel.x as usize] =
                pixel.position;
        }

        let lights: Vec<Light> = world
            .read::<Light>()
            .get::<Light>()
            .iter()
            .copied()
            .copied()
            .collect();

        // lighting pass
        let mut lighting_buffers = vec![vec![Color::BLACK; self.color_buffer.len()]; lights.len()];
        let camera = world.read_resource::<PerspectiveCamera>().unwrap();
        lighting_buffers
            .iter_mut()
            .enumerate()
            .for_each(|(i, buffer)| {
                let light = lights[i];
                let shader = shader::PhongFragmentShader {
                    light,
                    camera_position: camera.position(),
                    shininess: 1.0,
                };
                buffer.par_iter_mut().enumerate().for_each(|(i, color)| {
                    let normal = self.normal_buffer[i];
                    let position = self.position_buffer[i];
                    let depth = self.depth_buffer[i];

                    *color = shader.fragment_shader(position, normal, depth, *color);
                });
            });

        // add lights and multiply by unlit color
        self.color_buffer
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, color)| {
                let mut light_color = Color::BLACK;
                for buffer in lighting_buffers.iter() {
                    light_color += buffer[i];
                }
                *color *= light_color;
            });

        // copy the color buffer to the pixels frame
        self.pixels
            .frame_mut()
            .chunks_exact_mut(4)
            .zip(self.color_buffer.iter())
            .for_each(|(pixel, color)| {
                pixel[0] = (color.r * 255.0) as u8;
                pixel[1] = (color.g * 255.0) as u8;
                pixel[2] = (color.b * 255.0) as u8;
                pixel[3] = 255;
            });

        self.pixels.render().unwrap();
    }
}

impl Renderer for SoftwareRenderer {
    fn create(window: &winit::window::Window) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self::new(window))
    }

    fn render(&mut self, world: &mut World) -> anyhow::Result<()> {
        self.render_impl(world);
        Ok(())
    }
}
