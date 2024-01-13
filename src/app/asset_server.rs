use std::{path::PathBuf, sync::Arc};

use rustc_hash::FxHashMap;
use weaver_proc_macro::Resource;

use crate::{
    core::{
        material::Material,
        mesh::Mesh,
        texture::{HdrCubeTexture, NormalMapTexture, SdrTexture, Texture, TextureFormat},
    },
    ecs::World,
    renderer::{compute::hdr_loader::HdrLoader, internals::GpuResourceManager, Renderer},
};

#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct AssetId(pub u64);

impl AssetId {
    pub const PLACEHOLDER: Self = Self(u64::MAX);
}

impl Default for AssetId {
    fn default() -> Self {
        Self::PLACEHOLDER
    }
}

#[derive(Resource)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AssetServer {
    next_id: u64,
    path_prefix: PathBuf,
    ids: FxHashMap<PathBuf, AssetId>,
    #[cfg_attr(feature = "serde", serde(skip))]
    resource_manager: Arc<GpuResourceManager>,
    #[cfg_attr(feature = "serde", serde(skip))]
    meshes: FxHashMap<AssetId, Mesh>,
    #[cfg_attr(feature = "serde", serde(skip))]
    textures: FxHashMap<AssetId, Texture>,
    #[cfg_attr(feature = "serde", serde(skip))]
    materials: FxHashMap<AssetId, Material>,
}

impl AssetServer {
    pub fn new(world: &World) -> anyhow::Result<Self> {
        let renderer = world.read_resource::<Renderer>()?;
        let resource_manager = renderer.resource_manager().clone();
        Ok(Self {
            next_id: 0,
            path_prefix: PathBuf::from("assets"),
            ids: FxHashMap::default(),
            resource_manager,
            meshes: FxHashMap::default(),
            textures: FxHashMap::default(),
            materials: FxHashMap::default(),
        })
    }

    pub(crate) fn alloc_id(&mut self) -> AssetId {
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

    pub fn load_mesh(&mut self, path: impl Into<PathBuf>) -> anyhow::Result<Mesh> {
        let path = path.into();
        let path = if path.is_absolute() {
            path
        } else {
            self.path_prefix.join(path)
        };

        if !self.ids.contains_key(&path) {
            let id = self.alloc_id();
            if path.extension().unwrap() == "obj" {
                let mesh = Mesh::load_obj(path.clone(), self.resource_manager.device(), id)?;
                self.ids.insert(path.clone(), id);
                self.meshes.insert(id, mesh);
            } else {
                let mesh = Mesh::load_gltf(path.clone(), self.resource_manager.device(), id)?;
                self.ids.insert(path.clone(), id);
                self.meshes.insert(id, mesh);
            }
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

    pub fn load_texture(&mut self, path: impl Into<PathBuf>) -> anyhow::Result<Texture> {
        let path = path.into();
        let path = if path.is_absolute() {
            path
        } else {
            self.path_prefix.join(path)
        };

        if !self.ids.contains_key(&path) {
            let id = self.alloc_id();
            let texture = Texture::load(path.clone(), SdrTexture::FORMAT, None);
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

    pub fn load_normal_map(&mut self, path: impl Into<PathBuf>) -> anyhow::Result<Texture> {
        let path = path.into();
        let path = if path.is_absolute() {
            path
        } else {
            self.path_prefix.join(path)
        };

        if !self.ids.contains_key(&path) {
            let id = self.alloc_id();
            let texture = Texture::load(path.clone(), NormalMapTexture::FORMAT, None);
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

    pub fn load_hdr_cubemap(
        &mut self,
        path: impl Into<PathBuf>,
        dst_size: u32,
        hdr_loader: &HdrLoader,
    ) -> anyhow::Result<HdrCubeTexture> {
        let path = path.into();
        let path = if path.is_absolute() {
            path
        } else {
            self.path_prefix.join(path)
        };
        let texture = hdr_loader.load(&self.resource_manager, dst_size, path)?;
        Ok(texture)
    }

    pub fn create_material(
        &mut self,
        diffuse_texture: Option<SdrTexture>,
        normal_texture: Option<NormalMapTexture>,
        roughness_texture: Option<SdrTexture>,
        ambient_occlusion_texture: Option<SdrTexture>,
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
