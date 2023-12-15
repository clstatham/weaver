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
}
