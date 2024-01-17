use glam::{Vec2, Vec4};

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct ParticleUniform {
    position: Vec4,
    color: Vec4,
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct ParticleVertex {
    position: Vec4,
    uv: Vec2,
    _padding: [f32; 2],
}
