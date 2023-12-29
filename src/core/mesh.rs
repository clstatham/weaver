use weaver_proc_macro::Component;

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: glam::Vec3,
    pub normal: glam::Vec3,
    pub uv: glam::Vec2,
    pub tangent: glam::Vec3,
    pub bitangent: glam::Vec3,
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // normal
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<glam::Vec3>() as u64,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // uv
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<glam::Vec3>() * 2) as u64,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // tangent
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<glam::Vec3>() * 2
                        + std::mem::size_of::<glam::Vec2>()) as u64,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // bitangent
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<glam::Vec3>() * 3
                        + std::mem::size_of::<glam::Vec2>()) as u64,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

#[derive(Component)]
pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub(crate) num_indices: u32,
}
