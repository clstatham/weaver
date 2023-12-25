use self::color::Color;

pub mod camera;
pub mod color;
pub mod input;
pub mod light;
pub mod mesh;
pub mod texture;
pub mod transform;

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bytemuck::Pod,
    bytemuck::Zeroable,
)]
#[repr(C)]
pub struct Vertex {
    pub position: glam::Vec3,
    pub normal: glam::Vec3,
    pub color: Color,
    pub uv: glam::Vec2,
}

impl Vertex {
    pub fn new(position: glam::Vec3, normal: glam::Vec3, color: Color, uv: glam::Vec2) -> Self {
        Self {
            position,
            normal,
            color,
            uv,
        }
    }

    pub const fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    format: wgpu::VertexFormat::Float32x3,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<glam::Vec3>() as wgpu::BufferAddress,
                    format: wgpu::VertexFormat::Float32x3,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<glam::Vec3>() * 2) as wgpu::BufferAddress,
                    format: wgpu::VertexFormat::Float32x4,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<glam::Vec3>() * 2 + std::mem::size_of::<Color>())
                        as wgpu::BufferAddress,
                    format: wgpu::VertexFormat::Float32x2,
                    shader_location: 3,
                },
            ],
        }
    }
}
