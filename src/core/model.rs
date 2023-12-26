use std::path::Path;

use super::{mesh::Mesh, transform::Transform};

pub struct Model {
    pub mesh: Mesh,
    pub transform: Transform,
}

impl Model {
    pub fn new(mesh: Mesh, transform: Transform) -> Self {
        Self { mesh, transform }
    }

    pub fn load_gltf(path: impl AsRef<Path>, device: &wgpu::Device) -> anyhow::Result<Self> {
        let mesh = Mesh::load_gltf(path, device)?;
        let transform = Transform::new();

        Ok(Self::new(mesh, transform))
    }
}
