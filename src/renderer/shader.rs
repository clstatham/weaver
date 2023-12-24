use crate::core::{
    camera::PerspectiveCamera, color::Color, light::PointLight, transform::Transform, Vertex,
};

pub trait VertexShader
where
    Self: Send + Sync,
{
    fn vertex_shader(&self, vertex_in: Vertex) -> Vertex;
}

pub trait FragmentShader
where
    Self: Send + Sync,
{
    fn fragment_shader(&self, vertex_in: Vertex, color_in: Color) -> Color;
}

pub struct DefaultVertexShader;

impl VertexShader for DefaultVertexShader {
    fn vertex_shader(&self, vertex_in: Vertex) -> Vertex {
        vertex_in
    }
}

pub struct DefaultFragmentShader;

impl FragmentShader for DefaultFragmentShader {
    fn fragment_shader(&self, _vertex_in: Vertex, color_in: Color) -> Color {
        color_in
    }
}

pub struct ChainVertexShader(pub Vec<Box<dyn VertexShader>>);

impl VertexShader for ChainVertexShader {
    fn vertex_shader(&self, vertex_in: Vertex) -> Vertex {
        let mut vertex = vertex_in;

        for shader in self.0.iter() {
            vertex = shader.vertex_shader(vertex);
        }

        vertex
    }
}

pub struct ChainFragmentShader(pub Vec<Box<dyn FragmentShader>>);

impl FragmentShader for ChainFragmentShader {
    fn fragment_shader(&self, vertex_in: Vertex, color_in: Color) -> Color {
        let mut color = color_in;

        for shader in self.0.iter() {
            color = shader.fragment_shader(vertex_in, color);
        }

        color
    }
}

#[macro_export]
macro_rules! vertex_shader {
    ($($shader:expr),*) => {
        $crate::renderer::shader::ChainVertexShader(vec![$(Box::new($shader)),*])
    };
}

#[macro_export]
macro_rules! fragment_shader {
    ($($shader:expr),*) => {
        $crate::renderer::shader::ChainFragmentShader(vec![$(Box::new($shader)),*])
    };
}

pub struct TransformVertexShader {
    pub transform: Transform,
}

impl VertexShader for TransformVertexShader {
    fn vertex_shader(&self, vertex_in: Vertex) -> Vertex {
        let position = vertex_in.position;
        let normal = vertex_in.normal;
        let color = vertex_in.color;

        let (scale, rotation, translation) = self.transform.0.to_scale_rotation_translation();

        let position = rotation * position;
        let position = position * scale;
        let position = position + translation;

        let normal = (rotation * normal).normalize();

        Vertex {
            position,
            normal,
            color,
        }
    }
}

pub struct SolidColorFragmentShader {
    pub color: Color,
}

impl FragmentShader for SolidColorFragmentShader {
    fn fragment_shader(&self, _vertex_in: Vertex, _color_in: Color) -> Color {
        self.color
    }
}

pub struct VertexColorFragmentShader;

impl FragmentShader for VertexColorFragmentShader {
    fn fragment_shader(&self, vertex_in: Vertex, _color_in: Color) -> Color {
        vertex_in.color
    }
}

pub struct CameraProjection<'a> {
    pub camera: &'a PerspectiveCamera,
    pub screen_width: usize,
    pub screen_height: usize,
}

impl<'a> CameraProjection<'a> {
    pub fn new(camera: &'a PerspectiveCamera, screen_width: usize, screen_height: usize) -> Self {
        Self {
            camera,
            screen_width,
            screen_height,
        }
    }
}

impl<'a> VertexShader for CameraProjection<'a> {
    fn vertex_shader(&self, vertex_in: Vertex) -> Vertex {
        let position = vertex_in.position;
        let normal = vertex_in.normal;
        let color = vertex_in.color;

        let position = self.camera.world_to_projection(position);

        Vertex {
            position,
            normal,
            color,
        }
    }
}

pub struct PhongFragmentShader {
    pub lights: Vec<PointLight>,
    pub camera_position: glam::Vec3,
    pub shininess: f32,
}

impl PhongFragmentShader {
    pub fn new(lights: Vec<PointLight>, camera_position: glam::Vec3, shininess: f32) -> Self {
        Self {
            lights,
            camera_position,
            shininess,
        }
    }
}

impl FragmentShader for PhongFragmentShader {
    fn fragment_shader(&self, vertex_in: Vertex, color_in: Color) -> Color {
        let position = vertex_in.position;
        let normal = vertex_in.normal;
        let mut color = color_in;

        for light in self.lights.iter() {
            let light_direction = (light.position - position).normalize();
            let light_color = light.color;
            let light_intensity = light.intensity;

            let ambient = 0.1;
            let diffuse = (light_direction.dot(normal) * light_intensity).max(0.0);
            let specular = {
                let view_direction = (self.camera_position - position).normalize();
                let half_direction = (light_direction + view_direction).normalize();
                let specular = (half_direction.dot(normal) * light_intensity).max(0.0);
                specular.powf(self.shininess)
            };

            color *= light_color * (ambient + diffuse) + specular;
        }

        color
    }
}
