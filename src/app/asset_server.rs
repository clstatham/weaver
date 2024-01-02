use std::{path::PathBuf, sync::Arc};

use rustc_hash::FxHashMap;

use crate::core::{material::Material, mesh::Mesh, texture::Texture};

#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct AssetId(pub u64);

impl AssetId {
    pub const PLACEHOLDER: Self = Self(u64::MAX);
}

pub struct AssetServer {
    next_id: u64,
    ids: FxHashMap<PathBuf, AssetId>,
    meshes: FxHashMap<AssetId, Mesh>,
    materials: FxHashMap<AssetId, Material>,
    textures: FxHashMap<AssetId, Texture>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl AssetServer {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self {
            next_id: 0,
            ids: FxHashMap::default(),
            meshes: FxHashMap::default(),
            materials: FxHashMap::default(),
            textures: FxHashMap::default(),
            device,
            queue,
        }
    }

    fn alloc_id(&mut self) -> AssetId {
        let id = AssetId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn load_mesh(&mut self, path: impl Into<PathBuf>) -> anyhow::Result<Mesh> {
        let path = path.into();
        if !self.ids.contains_key(&path) {
            let id = self.alloc_id();
            let mesh = Mesh::load_gltf(path.clone(), self.device.as_ref(), id)?;
            self.ids.insert(path.clone(), id);
            self.meshes.insert(id, mesh);
        }
        Ok(self
            .ids
            .get(&path)
            .and_then(|id| self.meshes.get(id))
            .unwrap()
            .clone())
    }

    pub fn load_material(&mut self, path: impl Into<PathBuf>) -> anyhow::Result<Material> {
        let path = path.into();
        if !self.ids.contains_key(&path) {
            let id = self.alloc_id();
            let mut materials =
                Material::load_gltf(path.clone(), self.device.as_ref(), self.queue.as_ref())?;
            self.ids.insert(path.clone(), id);
            self.materials.insert(id, materials.remove(0));
        }
        Ok(self
            .ids
            .get(&path)
            .and_then(|id| self.materials.get(id))
            .unwrap()
            .clone())
    }

    pub fn load_texture(
        &mut self,
        path: impl Into<PathBuf>,
        is_normal_map: bool,
    ) -> anyhow::Result<Texture> {
        let path = path.into();
        if !self.ids.contains_key(&path) {
            let id = self.alloc_id();
            let texture = Texture::load(
                path.clone(),
                self.device.as_ref(),
                self.queue.as_ref(),
                None,
                is_normal_map,
            );
            self.ids.insert(path.clone(), id);
            self.textures.insert(id, texture);
        }
        Ok(self
            .ids
            .get(&path)
            .and_then(|id| self.textures.get(id))
            .unwrap()
            .clone())
    }

    pub fn create_material(
        &mut self,
        diffuse_texture: Option<Texture>,
        normal_texture: Option<Texture>,
        roughness_texture: Option<Texture>,
        ambient_occlusion_texture: Option<Texture>,
    ) -> Material {
        let id = self.alloc_id();
        let material = Material::new(
            diffuse_texture,
            normal_texture,
            roughness_texture,
            ambient_occlusion_texture,
            id,
        );
        self.materials.insert(id, material.clone());
        material
    }
}
