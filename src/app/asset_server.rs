use std::{path::PathBuf, rc::Rc};

use rustc_hash::FxHashMap;
use weaver_proc_macro::Resource;

use crate::{
    core::{
        material::Material,
        mesh::Mesh,
        texture::{HdrD2ArrayFormat, NormalMapFormat, SdrFormat, Texture, TextureFormat},
    },
    ecs::World,
    renderer::{compute::hdr_loader::HdrLoader, BufferAllocator, Renderer},
};

#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct AssetId(pub u64);

impl AssetId {
    pub const PLACEHOLDER: Self = Self(u64::MAX);
}

#[derive(Resource)]
pub struct AssetServer {
    next_id: u64,
    path_prefix: PathBuf,
    ids: FxHashMap<PathBuf, AssetId>,
    meshes: FxHashMap<AssetId, Mesh>,
    materials: FxHashMap<AssetId, Material>,
}

impl AssetServer {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            next_id: 0,
            path_prefix: PathBuf::from("assets"),
            ids: FxHashMap::default(),
            meshes: FxHashMap::default(),
            materials: FxHashMap::default(),
        })
    }

    fn alloc_id(&mut self) -> AssetId {
        let id = AssetId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn path_prefix(&self) -> &PathBuf {
        &self.path_prefix
    }

    pub fn set_path_prefix(&mut self, path_prefix: impl Into<PathBuf>) {
        self.path_prefix = path_prefix.into();
    }

    pub fn load_mesh(
        &mut self,
        path: impl Into<PathBuf>,
        renderer: &Renderer,
    ) -> anyhow::Result<Mesh> {
        let path = path.into();
        let path = if path.is_absolute() {
            path
        } else {
            self.path_prefix.join(path)
        };

        if !self.ids.contains_key(&path) {
            let id = self.alloc_id();
            let mesh = Mesh::load_gltf(path.clone(), &renderer.device, id)?;
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
        let path = if path.is_absolute() {
            path
        } else {
            self.path_prefix.join(path)
        };
        if !self.ids.contains_key(&path) {
            let id = self.alloc_id();
            let mut materials = Material::load_gltf(path.clone(), id)?;
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

    pub fn load_texture<F: TextureFormat>(
        &mut self,
        path: impl Into<PathBuf>,
    ) -> anyhow::Result<Texture<F>> {
        let path = path.into();
        let path = if path.is_absolute() {
            path
        } else {
            self.path_prefix.join(path)
        };
        let texture = Texture::load(path.clone(), None);
        Ok(texture)
    }

    pub fn load_hdr_cubemap(
        &mut self,
        path: impl Into<PathBuf>,
        dst_size: u32,
        renderer: &Renderer,
        hdr_loader: &HdrLoader,
    ) -> anyhow::Result<Texture<HdrD2ArrayFormat>> {
        let path = path.into();
        let path = if path.is_absolute() {
            path
        } else {
            self.path_prefix.join(path)
        };
        let texture = hdr_loader.load(renderer, dst_size, path)?;
        Ok(texture)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_material(
        &mut self,
        diffuse_texture: Option<Texture<SdrFormat>>,
        normal_texture: Option<Texture<NormalMapFormat>>,
        roughness_texture: Option<Texture<SdrFormat>>,
        ambient_occlusion_texture: Option<Texture<SdrFormat>>,
        metallic: Option<f32>,
        roughness: Option<f32>,
        texture_scaling: Option<f32>,
    ) -> Material {
        let id = self.alloc_id();
        let material = Material::new(
            diffuse_texture,
            normal_texture,
            roughness_texture,
            ambient_occlusion_texture,
            metallic,
            roughness,
            texture_scaling,
            id,
        );
        self.materials.insert(id, material.clone());
        material
    }
}
