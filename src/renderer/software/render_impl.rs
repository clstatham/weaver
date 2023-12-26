use super::{
    shader::{self, VertexShader},
    Pixel, SoftwareRenderer,
};
use crate::core::{camera::PerspectiveCamera, texture::Texture, Vertex};

impl SoftwareRenderer {
    /// Draws a filled triangle between three world-space vertices using the barycentric method.
    #[allow(clippy::too_many_arguments)]
    pub fn triangle<V: VertexShader>(
        &self,
        v0: Vertex,
        v1: Vertex,
        v2: Vertex,
        vertex_shader: &V,
        texture: Option<&Texture>,
        camera: &PerspectiveCamera,
    ) -> Vec<Pixel> {
        let camera_shader =
            shader::CameraProjection::new(camera, self.screen_width, self.screen_height);
        let v0 = camera_shader.vertex_shader(vertex_shader.vertex_shader(v0));
        let v1 = camera_shader.vertex_shader(vertex_shader.vertex_shader(v1));
        let v2 = camera_shader.vertex_shader(vertex_shader.vertex_shader(v2));

        let (x0, y0) = self.view_to_screen((v0.position.x, v0.position.y));
        let (x1, y1) = self.view_to_screen((v1.position.x, v1.position.y));
        let (x2, y2) = self.view_to_screen((v2.position.x, v2.position.y));

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

        // let mut pixels = Vec::new();

        (y_min..=y_max)
            .flat_map(|y| {
                (x_min..=x_max).filter_map(move |x| {
                    let w0 = ((y1 - y2) * (x - x2) + (x2 - x1) * (y - y2)) as f32
                        / ((y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2)) as f32;
                    let w1 = ((y2 - y0) * (x - x2) + (x0 - x2) * (y - y2)) as f32
                        / ((y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2)) as f32;

                    if w0 >= 0.0 && w1 >= 0.0 && w0 + w1 <= 1.0 {
                        let w2 = 1.0 - w0 - w1;

                        let z = w0 * v0.position.z + w1 * v1.position.z + w2 * v2.position.z;
                        if z < self.get_depth(x as usize, y as usize) {
                            let position = v0.position * w0 + v1.position * w1 + v2.position * w2;
                            let normal = v0.normal * w0 + v1.normal * w1 + v2.normal * w2;
                            let uv = v0.uv * w0 + v1.uv * w1 + v2.uv * w2;
                            let color = v0.color * w0 + v1.color * w1 + v2.color * w2;

                            let color = if let Some(texture) = texture {
                                if let Some(texture_color) = texture.get_uv(uv.x, uv.y) {
                                    // self.set_color(x as usize, y as usize, color * texture_color);
                                    color * texture_color
                                } else {
                                    color
                                }
                            } else {
                                color
                            };

                            return Some(Pixel {
                                x,
                                y,
                                color,
                                position,
                                normal,
                                depth: z,
                            });
                        }
                    }

                    None
                })
            })
            .collect()
    }
}
