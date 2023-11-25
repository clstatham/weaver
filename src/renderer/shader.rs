use super::{camera::PerspectiveCamera, color::Color, light::PointLight, mesh::Vertex};

pub trait VertexShader {
    fn vertex_shader(&self, vertex_in: Vertex) -> Vertex;
}

pub trait FragmentShader {
    fn fragment_shader(&self, vertex_in: Vertex, color_in: Color) -> Color;
}

pub struct DummyShader;

impl DummyShader {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DummyShader {
    fn default() -> Self {
        Self::new()
    }
}

impl VertexShader for DummyShader {
    fn vertex_shader(&self, vertex_in: Vertex) -> Vertex {
        vertex_in
    }
}

impl FragmentShader for DummyShader {
    fn fragment_shader(&self, _vertex_in: Vertex, color_in: Color) -> Color {
        color_in
    }
}

pub struct ChainVertexShader {
    pub shaders: Vec<Box<dyn VertexShader>>,
}

impl ChainVertexShader {
    pub fn new(shaders: Vec<Box<dyn VertexShader>>) -> Self {
        Self { shaders }
    }
}

impl VertexShader for ChainVertexShader {
    fn vertex_shader(&self, vertex_in: Vertex) -> Vertex {
        let mut vertex = vertex_in;
        for shader in &self.shaders {
            vertex = shader.vertex_shader(vertex);
        }
        vertex
    }
}

pub struct ChainFragmentShader {
    pub shaders: Vec<Box<dyn FragmentShader>>,
}

impl ChainFragmentShader {
    pub fn new(shaders: Vec<Box<dyn FragmentShader>>) -> Self {
        Self { shaders }
    }
}

impl FragmentShader for ChainFragmentShader {
    fn fragment_shader(&self, vertex_in: Vertex, color_in: Color) -> Color {
        let mut color = color_in;
        for shader in &self.shaders {
            color = shader.fragment_shader(vertex_in, color);
        }
        color
    }
}

#[macro_export]
macro_rules! chain_vs {
    ($($shader:expr),*) => {
        ChainVertexShader::new(vec![$(Box::new($shader)),*])
    };
}

#[macro_export]
macro_rules! chain_fs {
    ($($shader:expr),*) => {
        ChainFragmentShader::new(vec![$(Box::new($shader)),*])
    };
}

pub struct TransformVertexShader {
    pub translation: glam::Vec3,
    pub rotation: glam::Vec3,
    pub scale: glam::Vec3,
    pub transform: glam::Mat4,
}

impl TransformVertexShader {
    pub fn new(translation: glam::Vec3, rotation: glam::Vec3, scale: glam::Vec3) -> Self {
        Self {
            translation,
            rotation,
            scale,
            transform: glam::Mat4::from_scale_rotation_translation(
                scale,
                glam::Quat::from_euler(glam::EulerRot::XYZ, rotation.x, rotation.y, rotation.z),
                translation,
            ),
        }
    }
}

impl VertexShader for TransformVertexShader {
    fn vertex_shader(&self, vertex_in: Vertex) -> Vertex {
        let new_pos = self.transform.transform_point3(vertex_in.position);
        let new_normal = vertex_in
            .normal
            .map(|normal| self.transform.transform_vector3(normal).normalize());
        Vertex {
            position: new_pos,
            color: vertex_in.color,
            normal: new_normal,
        }
    }
}

pub struct CameraProjection<'a> {
    pub camera: &'a PerspectiveCamera,
    pub screen_size: (u32, u32),
}

impl<'a> CameraProjection<'a> {
    pub fn new(camera: &'a PerspectiveCamera, screen_size: (u32, u32)) -> Self {
        Self {
            camera,
            screen_size,
        }
    }
}

impl<'a> VertexShader for CameraProjection<'a> {
    fn vertex_shader(&self, mut vertex_in: Vertex) -> Vertex {
        vertex_in.position = self
            .camera
            .world_to_screen(self.screen_size, vertex_in.position);

        vertex_in
    }
}

pub struct SolidColor {
    pub color: Color,
}

impl SolidColor {
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl FragmentShader for SolidColor {
    fn fragment_shader(&self, _vertex_in: Vertex, _color_in: Color) -> Color {
        self.color
    }
}

pub struct Diffuse {
    pub lights: Vec<PointLight>,
}

impl Diffuse {
    pub fn new(lights: &[PointLight]) -> Self {
        Self {
            lights: lights.to_vec(),
        }
    }
}

impl FragmentShader for Diffuse {
    fn fragment_shader(&self, vertex_in: Vertex, color_in: Color) -> Color {
        let mut color = color_in;

        for light in &self.lights {
            let light_dir = (light.position - vertex_in.position).normalize();
            let normal = vertex_in.normal.unwrap_or(glam::Vec3::Y);
            let light_intensity = light.intensity * normal.dot(light_dir).max(0.0);

            color = color * light.color * light_intensity;
        }

        color
    }
}

pub struct Specular {
    pub lights: Vec<PointLight>,
    pub shininess: f32,
}

impl Specular {
    pub fn new(lights: &[PointLight], shininess: f32) -> Self {
        Self {
            lights: lights.to_vec(),
            shininess,
        }
    }
}

impl FragmentShader for Specular {
    fn fragment_shader(&self, vertex_in: Vertex, color_in: Color) -> Color {
        let mut color = color_in;

        for light in &self.lights {
            let light_dir = (light.position - vertex_in.position).normalize();
            let normal = vertex_in.normal.unwrap_or(glam::Vec3::Y);
            let light_intensity = light.intensity * normal.dot(light_dir).max(0.0);

            let view_dir = -vertex_in.position.normalize();
            let half_dir = (light_dir + view_dir).normalize();
            let specular = half_dir.dot(normal).max(0.0).powf(self.shininess);

            color = color * light.color * light_intensity * specular;
        }

        color
    }
}
