use rustc_hash::FxHashMap;

use crate::ecs::{
    component::Field,
    system::{Query, ResolvedQuery},
    world::World,
};

use self::{
    camera::PerspectiveCamera,
    color::Color,
    mesh::{Mesh, Vertex},
    shader::{DummyShader, FragmentShader, VertexShader},
};

pub mod camera;
pub mod color;
pub mod mesh;
#[macro_use]
pub mod shader;

pub struct Renderer {
    pub screen_width: u32,
    pub screen_height: u32,
    pub color_buffer: Vec<Color>,
    pub depth_buffer: Vec<f32>,
    pub camera: PerspectiveCamera,
}

impl Renderer {
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        let aspect = screen_width as f32 / screen_height as f32;
        let mut camera = PerspectiveCamera::new();
        camera.aspect = aspect;
        camera.position = glam::Vec3::new(0.0, 0.0, 1.0);
        Self {
            screen_width,
            screen_height,
            color_buffer: vec![Color::new(0.0, 0.0, 0.0); (screen_width * screen_height) as usize],
            depth_buffer: vec![0.0; (screen_width * screen_height) as usize],
            camera,
        }
    }

    pub fn set_color(&mut self, x: u32, y: u32, color: Color) {
        let offset = x + y * self.screen_width;
        if offset >= self.color_buffer.len() as u32 {
            return;
        }
        self.color_buffer[offset as usize] = color;
    }

    pub fn set_depth(&mut self, x: u32, y: u32, depth: f32) {
        let offset = x + y * self.screen_width;
        if offset >= self.depth_buffer.len() as u32 {
            return;
        }
        self.depth_buffer[offset as usize] = depth;
    }

    pub fn get_color(&self, x: u32, y: u32) -> Option<Color> {
        let offset = x + y * self.screen_width;
        if offset >= self.color_buffer.len() as u32 {
            return None;
        }
        Some(self.color_buffer[offset as usize])
    }

    pub fn get_depth(&self, x: u32, y: u32) -> Option<f32> {
        let offset = x + y * self.screen_width;
        if offset >= self.depth_buffer.len() as u32 {
            return None;
        }
        Some(self.depth_buffer[offset as usize])
    }

    /// Draws a line between two vertices, applying the given vertex and fragment shaders.
    pub fn line<V: VertexShader, F: FragmentShader>(
        &mut self,
        v0: Vertex,
        v1: Vertex,
        vertex_shader: &V,
        fragment_shader: &F,
    ) {
        let camera_shader =
            shader::CameraProjection::new(&self.camera, (self.screen_width, self.screen_height));
        let v0 = camera_shader.vertex_shader(vertex_shader.vertex_shader(v0));
        let v1 = camera_shader.vertex_shader(vertex_shader.vertex_shader(v1));

        let x0 = v0.position.x as i32;
        let y0 = v0.position.y as i32;
        let x1 = v1.position.x as i32;
        let y1 = v1.position.y as i32;

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
                self.set_depth(x as u32, y as u32, depth);
                self.set_color(x as u32, y as u32, color);

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
                self.set_depth(x as u32, y as u32, depth);
                self.set_color(x as u32, y as u32, color);

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
    #[allow(clippy::too_many_arguments)]
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

    pub fn filled_triangle<V: VertexShader, F: FragmentShader>(
        &mut self,
        v0: Vertex,
        v1: Vertex,
        v2: Vertex,
        vertex_shader: &V,
        fragment_shader: &F,
    ) {
        let camera_shader =
            shader::CameraProjection::new(&self.camera, (self.screen_width, self.screen_height));
        let v0 = camera_shader.vertex_shader(vertex_shader.vertex_shader(v0));
        let v1 = camera_shader.vertex_shader(vertex_shader.vertex_shader(v1));
        let v2 = camera_shader.vertex_shader(vertex_shader.vertex_shader(v2));

        let x0 = v0.position.x as i32;
        let y0 = v0.position.y as i32;
        let x1 = v1.position.x as i32;
        let y1 = v1.position.y as i32;
        let x2 = v2.position.x as i32;
        let y2 = v2.position.y as i32;

        // calculate triangle bounding box
        let min_x = x0.min(x1).min(x2);
        let min_y = y0.min(y1).min(y2);
        let max_x = x0.max(x1).max(x2);
        let max_y = y0.max(y1).max(y2);

        // clip against screen bounds
        let min_x = min_x.max(0);
        let min_y = min_y.max(0);
        let max_x = max_x.min(self.screen_width as i32 - 1);
        let max_y = max_y.min(self.screen_height as i32 - 1);

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                // calculate barycentric coordinates
                let w0 = ((y1 - y2) * (x - x2) + (x2 - x1) * (y - y2)) as f32
                    / ((y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2)) as f32;
                let w1 = ((y2 - y0) * (x - x2) + (x0 - x2) * (y - y2)) as f32
                    / ((y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2)) as f32;
                let w2 = 1.0 - w0 - w1;

                // if point is in triangle, draw it
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    // interpolate the vertex using the barycentric coordinates
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
                    // let vertex = Vertex { position, color };

                    // depth test
                    let depth = w0 * v0.position.z + w1 * v1.position.z + w2 * v2.position.z;
                    let current_depth = self.get_depth(x as u32, y as u32).unwrap_or(0.0);
                    if depth > current_depth {
                        continue;
                    }

                    self.set_depth(x as u32, y as u32, depth);
                    // let color = Color::new(1.0, 1.0, 1.0);
                    // let color = fragment_shader.fragment_shader(vertex, color);
                    self.set_color(x as u32, y as u32, color);
                }
            }
        }
    }

    /// Renders the given [World] to the given frame.
    pub fn render(&mut self, frame: &mut [u8], world: &World) -> anyhow::Result<()> {
        // Clear color buffer
        self.color_buffer.fill(Color::new(0.1, 0.1, 0.1));
        // Clear depth buffer
        self.depth_buffer.fill(f32::INFINITY);

        // Query the world for meshes and transforms.
        let query = Query::Immutable("mesh".to_string());
        let meshes = world.query(&query);
        let query = Query::Immutable("transform".to_string());
        let transforms = world.query(&query);
        let mut transformed_meshes = FxHashMap::default();

        if let (ResolvedQuery::Immutable(meshes), ResolvedQuery::Immutable(transforms)) =
            (meshes, transforms)
        {
            // Transform the meshes, storing the transformed meshes in a map.
            for mesh_comp in meshes {
                let mesh = match mesh_comp.fields.get("mesh") {
                    Some(Field::Mesh(mesh)) => mesh,
                    _ => {
                        log::error!("mesh component does not have a mesh field");
                        continue;
                    }
                };

                let transform_comp = match transforms
                    .iter()
                    .find(|transform| transform.entity() == mesh_comp.entity())
                {
                    Some(transform) => transform,
                    None => {
                        log::error!("mesh component does not have a transform component");
                        continue;
                    }
                };

                let position = match transform_comp.fields.get("position") {
                    Some(Field::Vec3(position)) => position,
                    _ => {
                        log::error!("transform component does not have a position field");
                        continue;
                    }
                };

                let rotation = match transform_comp.fields.get("rotation") {
                    Some(Field::Vec3(rotation)) => rotation,
                    _ => {
                        log::error!("transform component does not have a rotation field");
                        continue;
                    }
                };

                let scale = match transform_comp.fields.get("scale") {
                    Some(Field::Vec3(scale)) => scale,
                    _ => {
                        log::error!("transform component does not have a scale field");
                        continue;
                    }
                };

                let mut transformed_vertices = Vec::new();
                let transform = glam::Mat4::from_scale_rotation_translation(
                    *scale,
                    glam::Quat::from_euler(glam::EulerRot::XYZ, rotation.x, rotation.y, rotation.z),
                    *position,
                );
                for vertex in &mesh.vertices {
                    let transformed_position = transform.transform_point3(vertex.position);
                    let transformed_vertex = Vertex {
                        position: transformed_position,
                        color: vertex.color,
                    };

                    transformed_vertices.push(transformed_vertex);
                }

                transformed_meshes.insert(
                    mesh_comp.entity(),
                    Mesh::new(transformed_vertices, mesh.indices.clone()),
                );
            }

            // Render the transformed meshes.
            for mesh in transformed_meshes.values() {
                for i in (0..mesh.indices.len()).step_by(3) {
                    let i0 = mesh.indices[i] as usize;
                    let i1 = mesh.indices[i + 1] as usize;
                    let i2 = mesh.indices[i + 2] as usize;
                    let v0 = mesh.vertices[i0];
                    let v1 = mesh.vertices[i1];
                    let v2 = mesh.vertices[i2];

                    self.filled_triangle(v0, v1, v2, &DummyShader::new(), &DummyShader::new());
                }
            }
        }

        // Copy the color buffer to the frame in RGBA order.
        for (i, color) in self.color_buffer.iter().enumerate() {
            let offset = i * 4;
            frame[offset] = (color.r * 255.0) as u8;
            frame[offset + 1] = (color.g * 255.0) as u8;
            frame[offset + 2] = (color.b * 255.0) as u8;
            frame[offset + 3] = 255;
        }

        Ok(())
    }
}
