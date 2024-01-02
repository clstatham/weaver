use std::path::Path;

use rustc_hash::FxHashMap;
use weaver_proc_macro::Bundle;
use wgpu::util::DeviceExt;

use crate::renderer::Renderer;

use super::{
    material::Material,
    mesh::{Mesh, Vertex},
    texture::Texture,
    transform::Transform,
};

#[derive(Bundle)]
pub struct Model {
    pub mesh: Mesh,
    pub transform: Transform,
    pub material: Material,
}
