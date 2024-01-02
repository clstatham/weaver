use std::{path::PathBuf, sync::Arc};

use rustc_hash::FxHashMap;

use crate::core::{material::Material, mesh::Mesh, texture::Texture};

pub struct AssetServer {
    meshes: FxHashMap<PathBuf, Mesh>,
    materials: FxHashMap<PathBuf, Material>,
    textures: FxHashMap<PathBuf, Texture>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl AssetServer {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self {
            meshes: FxHashMap::default(),
            materials: FxHashMap::default(),
            textures: FxHashMap::default(),
            device,
            queue,
        }
    }

    pub fn load_mesh(&mut self, path: impl Into<PathBuf>) -> anyhow::Result<Mesh> {
        let path = path.into();
        if !self.meshes.contains_key(&path) {
            let mesh = Mesh::load_gltf(path.clone(), self.device.as_ref(), self.queue.as_ref())?;
            self.meshes.insert(path.clone(), mesh);
        }
        Ok(self.meshes.get(&path).unwrap().clone())
    }

    pub fn load_material(&mut self, path: impl Into<PathBuf>) -> anyhow::Result<Material> {
        let path = path.into();
        if !self.materials.contains_key(&path) {
            let mut material =
                Material::load_gltf(path.clone(), self.device.as_ref(), self.queue.as_ref())?;
            // todo: handle multiple materials per file
            self.materials.insert(path.clone(), material.remove(0));
        }
        Ok(self.materials.get(&path).unwrap().clone())
    }

    pub fn load_texture(
        &mut self,
        path: impl Into<PathBuf>,
        is_normal_map: bool,
    ) -> anyhow::Result<Texture> {
        let path = path.into();
        if !self.textures.contains_key(&path) {
            let texture = Texture::load(
                path.clone(),
                self.device.as_ref(),
                self.queue.as_ref(),
                None,
                is_normal_map,
            );
            self.textures.insert(path.clone(), texture);
        }
        Ok(self.textures.get(&path).unwrap().clone())
    }
}
