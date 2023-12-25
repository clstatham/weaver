use crate::core::{
    camera::PerspectiveCamera,
    color::Color,
    light::{Light, PointLight},
    texture::Texture,
    transform::Transform,
    Vertex,
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
    fn fragment_shader(
        &self,
        position_in: glam::Vec3,
        normal_in: glam::Vec3,
        depth_in: f32,
        color_in: Color,
    ) -> Color;
}

pub struct DefaultVertexShader;

impl VertexShader for DefaultVertexShader {
    fn vertex_shader(&self, vertex_in: Vertex) -> Vertex {
        vertex_in
    }
}

pub struct DefaultFragmentShader;

impl FragmentShader for DefaultFragmentShader {
    fn fragment_shader(
        &self,
        _position_in: glam::Vec3,
        _normal_in: glam::Vec3,
        _depth_in: f32,
        color_in: Color,
    ) -> Color {
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

pub struct ChainFragmentShader<'a>(pub Vec<Box<dyn FragmentShader + 'a>>);

impl<'a> FragmentShader for ChainFragmentShader<'a> {
    fn fragment_shader(
        &self,
        position_in: glam::Vec3,
        normal_in: glam::Vec3,
        depth_in: f32,
        color_in: Color,
    ) -> Color {
        let mut color = color_in;

        for shader in self.0.iter() {
            color = shader.fragment_shader(position_in, normal_in, depth_in, color);
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
            uv: vertex_in.uv,
        }
    }
}

pub struct SolidColorFragmentShader {
    pub color: Color,
}

impl FragmentShader for SolidColorFragmentShader {
    fn fragment_shader(
        &self,
        position_in: glam::Vec3,
        normal_in: glam::Vec3,
        depth_in: f32,
        color_in: Color,
    ) -> Color {
        self.color
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
            uv: vertex_in.uv,
        }
    }
}

pub struct PhongFragmentShader {
    pub light: Light,
    pub camera_position: glam::Vec3,
    pub shininess: f32,
}

impl PhongFragmentShader {
    pub fn new(light: Light, camera_position: glam::Vec3, shininess: f32) -> Self {
        Self {
            light,
            camera_position,
            shininess,
        }
    }
}

impl FragmentShader for PhongFragmentShader {
    fn fragment_shader(
        &self,
        position_in: glam::Vec3,
        normal_in: glam::Vec3,
        _depth_in: f32,
        color_in: Color,
    ) -> Color {
        let light_direction = match self.light {
            Light::Directional(ref directional_light) => directional_light.direction,
            Light::Point(ref point_light) => point_light.position - position_in,
            Light::Spot(ref spot_light) => spot_light.position - position_in,
        };

        let light_direction = light_direction.normalize();

        // if it's a spot light, check if the fragment is within the cone
        if let Light::Spot(ref spot_light) = self.light {
            let light_direction = -light_direction;
            let spot_direction = spot_light.direction.normalize();
            let spot_angle = spot_light.angle.to_radians();
            let spot_cos = spot_angle.cos();

            if light_direction.dot(spot_direction) < spot_cos {
                return color_in;
            }
        }

        let normal = normal_in.normalize();

        let diffuse = match self.light {
            Light::Directional(ref directional_light) => {
                let intensity = light_direction.dot(normal).max(0.0);
                directional_light.color * directional_light.intensity * intensity
            }
            Light::Point(ref point_light) => {
                let intensity = light_direction.dot(normal).max(0.0);
                point_light.color * point_light.intensity * intensity
            }
            Light::Spot(ref spot_light) => {
                let intensity = light_direction.dot(normal).max(0.0);
                spot_light.color * spot_light.intensity * intensity
            }
        };

        let specular = match self.light {
            Light::Directional(ref directional_light) => {
                let light_direction = -light_direction;
                let camera_direction = (self.camera_position - position_in).normalize();
                let half_direction = (light_direction + camera_direction).normalize();

                let intensity = half_direction.dot(normal).max(0.0).powf(self.shininess);
                directional_light.color * directional_light.intensity * intensity
            }
            Light::Point(ref point_light) => {
                let light_direction = -light_direction;
                let camera_direction = (self.camera_position - position_in).normalize();
                let half_direction = (light_direction + camera_direction).normalize();

                let intensity = half_direction.dot(normal).max(0.0).powf(self.shininess);
                point_light.color * point_light.intensity * intensity
            }
            Light::Spot(ref spot_light) => {
                let light_direction = -light_direction;
                let camera_direction = (self.camera_position - position_in).normalize();
                let half_direction = (light_direction + camera_direction).normalize();

                let intensity = half_direction.dot(normal).max(0.0).powf(self.shininess);
                spot_light.color * spot_light.intensity * intensity
            }
        };

        color_in * diffuse + specular
    }
}
