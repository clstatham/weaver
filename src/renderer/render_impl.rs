use crate::core::{color::Color, Vertex};

use super::{
    shader::{self, FragmentShader, VertexShader},
    Renderer,
};

impl Renderer {
    /// Draws a line between two vertices, applying the given vertex and fragment shaders.
    pub fn line<V: VertexShader, F: FragmentShader>(
        &mut self,
        v0: Vertex,
        v1: Vertex,
        vertex_shader: &V,
        fragment_shader: &F,
    ) {
        let camera_shader =
            shader::CameraProjection::new(&self.camera, self.screen_width, self.screen_height);
        let v0 = camera_shader.vertex_shader(vertex_shader.vertex_shader(v0));
        let v1 = camera_shader.vertex_shader(vertex_shader.vertex_shader(v1));

        let (x0, y0) = self.view_to_screen((v0.position.x, v0.position.y));
        let (x1, y1) = self.view_to_screen((v1.position.x, v1.position.y));

        let x0 = x0 as i32;
        let y0 = y0 as i32;
        let x1 = x1 as i32;
        let y1 = y1 as i32;

        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut x = x0;
        let mut y = y0;

        let mut depth = v0.position.z;

        let color0 = fragment_shader.fragment_shader(v0, v0.color);
        let color1 = fragment_shader.fragment_shader(v1, v1.color);
        let mut color = color0;

        if dx > dy {
            let depth_step = (v1.position.z - v0.position.z) / (dx as f32).max(1.0);
            let color_step = (color1 - color0) / (dx as f32).max(1.0);
            for _ in 0..dx {
                self.set_depth(x as usize, y as usize, depth);
                self.set_color(x as usize, y as usize, color);

                depth += depth_step;
                color += color_step;

                let e2 = 2 * err;
                if e2 >= dy {
                    err += dy;
                    x += sx;
                }
                if e2 <= dx {
                    err += dx;
                    y += sy;
                }
            }
        } else {
            let depth_step = (v1.position.z - v0.position.z) / (dy as f32).max(1.0);
            let color_step = (color1 - color0) / (dy as f32).max(1.0);
            for _ in 0..dy {
                self.set_depth(x as usize, y as usize, depth);
                self.set_color(x as usize, y as usize, color);

                depth += depth_step;
                color += color_step;

                let e2 = 2 * err;
                if e2 <= dx {
                    err += dx;
                    y += sy;
                }
                if e2 >= dy {
                    err += dy;
                    x += sx;
                }
            }
        }
    }

    /// Draws a wireframe triangle between three world-space vertices.
    pub fn wireframe_triangle<V: VertexShader, F: FragmentShader>(
        &mut self,
        v0: Vertex,
        v1: Vertex,
        v2: Vertex,
        vertex_shader: &V,
        fragment_shader: &F,
    ) {
        self.line(v0, v1, vertex_shader, fragment_shader);
        self.line(v1, v2, vertex_shader, fragment_shader);
        self.line(v2, v0, vertex_shader, fragment_shader);
    }

    /// Draws a filled triangle between three world-space vertices using the barycentric method.
    pub fn triangle<V: VertexShader, F: FragmentShader>(
        &mut self,
        v0: Vertex,
        v1: Vertex,
        v2: Vertex,
        vertex_shader: &V,
        fragment_shader: &F,
    ) {
        let camera_shader =
            shader::CameraProjection::new(&self.camera, self.screen_width, self.screen_height);
        let v0 = camera_shader.vertex_shader(vertex_shader.vertex_shader(v0));
        let v1 = camera_shader.vertex_shader(vertex_shader.vertex_shader(v1));
        let v2 = camera_shader.vertex_shader(vertex_shader.vertex_shader(v2));

        let (x0, y0) = self.view_to_screen((v0.position.x, v0.position.y));
        let (x1, y1) = self.view_to_screen((v1.position.x, v1.position.y));
        let (x2, y2) = self.view_to_screen((v2.position.x, v2.position.y));

        let x0 = x0 as i32;
        let y0 = y0 as i32;
        let x1 = x1 as i32;
        let y1 = y1 as i32;
        let x2 = x2 as i32;
        let y2 = y2 as i32;

        let mut x_min = x0.min(x1).min(x2);
        let mut y_min = y0.min(y1).min(y2);
        let mut x_max = x0.max(x1).max(x2);
        let mut y_max = y0.max(y1).max(y2);

        if x_min < 0 {
            x_min = 0;
        }
        if y_min < 0 {
            y_min = 0;
        }
        if x_max >= self.screen_width as i32 {
            x_max = self.screen_width as i32 - 1;
        }
        if y_max >= self.screen_height as i32 {
            y_max = self.screen_height as i32 - 1;
        }

        for y in y_min..=y_max {
            for x in x_min..=x_max {
                let w0 = ((y1 - y2) * (x - x2) + (x2 - x1) * (y - y2)) as f32
                    / ((y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2)) as f32;
                let w1 = ((y2 - y0) * (x - x2) + (x0 - x2) * (y - y2)) as f32
                    / ((y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2)) as f32;

                if w0 >= 0.0 && w1 >= 0.0 && w0 + w1 <= 1.0 {
                    let w2 = 1.0 - w0 - w1;

                    let z = w0 * v0.position.z + w1 * v1.position.z + w2 * v2.position.z;
                    if z < self.get_depth(x as usize, y as usize) {
                        let position = glam::Vec3::new(
                            w0 * v0.position.x + w1 * v1.position.x + w2 * v2.position.x,
                            w0 * v0.position.y + w1 * v1.position.y + w2 * v2.position.y,
                            w0 * v0.position.z + w1 * v1.position.z + w2 * v2.position.z,
                        );
                        let color = Color::new(
                            w0 * v0.color.r + w1 * v1.color.r + w2 * v2.color.r,
                            w0 * v0.color.g + w1 * v1.color.g + w2 * v2.color.g,
                            w0 * v0.color.b + w1 * v1.color.b + w2 * v2.color.b,
                        );

                        let normal = glam::Vec3::new(
                            w0 * v0.normal.x + w1 * v1.normal.x + w2 * v2.normal.x,
                            w0 * v0.normal.y + w1 * v1.normal.y + w2 * v2.normal.y,
                            w0 * v0.normal.z + w1 * v1.normal.z + w2 * v2.normal.z,
                        );

                        let vertex = Vertex {
                            position,
                            color,
                            normal,
                        };

                        let color = v0.color * w0 + v1.color * w1 + v2.color * w2;
                        let color = fragment_shader.fragment_shader(vertex, color);

                        self.set_depth(x as usize, y as usize, z);
                        self.set_color(x as usize, y as usize, color);
                    }
                }
            }
        }
    }
}
