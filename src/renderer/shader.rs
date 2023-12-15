use crate::core::{camera::PerspectiveCamera, color::Color, transform::Transform, Vertex};

pub trait VertexShader {
    fn vertex_shader(&self, vertex_in: Vertex) -> Vertex;
}

pub trait FragmentShader {
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
