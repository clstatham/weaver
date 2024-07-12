use std::{
    any::TypeId,
    collections::HashSet,
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use weaver_ecs::{
    component::{Res, ResMut},
    prelude::Resource,
    system::{SystemAccess, SystemParam},
    world::{FromWorld, UnsafeWorldCell, World},
};
use weaver_util::{
    lock::Lock,
    {anyhow, Result},
};
use zip::ZipArchive;

use crate::Asset;

pub trait Loadable {}

impl<T: Asset> Loadable for T {}
impl<T: Asset> Loadable for Vec<T> {}

/// Virtual filesystem created from one or more directories or archives.
#[derive(Default, Resource)]
pub struct Filesystem {
    roots: Vec<PathBuf>,
    archives: Vec<Lock<ZipArchive<BufReader<File>>>>,
}

impl Filesystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_root(mut self, root: impl AsRef<Path>) -> Self {
        self.add_root(root);
        self
    }

    pub fn with_archive(mut self, archive: impl AsRef<Path>) -> Result<Self> {
        self.add_archive(archive)?;
        Ok(self)
    }

    pub fn add_root(&mut self, root: impl AsRef<Path>) {
        self.roots.push(root.as_ref().to_path_buf());
    }

    pub fn add_archive(&mut self, archive: impl AsRef<Path>) -> Result<()> {
        let archive = File::open(archive)?;
        let archive = BufReader::new(archive);
        let archive = ZipArchive::new(archive)?;
        self.archives.push(Lock::new(archive));
        Ok(())
    }

    pub fn read_dir(&self, dir_path: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
        let dir_path = dir_path.as_ref();
        for root in self.roots.iter() {
            let full_path = root.join(dir_path);
            if full_path.exists() {
                let mut entries = std::fs::read_dir(full_path)?
                    .map(|entry| entry.map(|e| e.path()))
                    .collect::<Result<Vec<_>, _>>()?;
                entries.sort();
                return Ok(entries);
            }
        }

        let mut files = Vec::new();

        for archive in self.archives.iter() {
            let mut archive = archive.write();

            for i in 0..archive.len() {
                let file = archive.by_index(i)?;
                let path = file.enclosed_name().unwrap();
                if path.starts_with(dir_path) {
                    files.push(path);
                }
            }
        }

        if !files.is_empty() {
            return Ok(files);
        }

        Err(anyhow!("Failed to list directory: {:?}", dir_path))
    }

    pub fn exists(&self, path: impl AsRef<Path>) -> bool {
        let path = path.as_ref();
        for root in self.roots.iter() {
            let full_path = root.join(path);
            if full_path.exists() {
                return true;
            }
        }

        for archive in self.archives.iter() {
            let mut archive = archive.write();
            if archive.by_name(path.as_os_str().to_str().unwrap()).is_ok() {
                return true;
            };
        }

        false
    }

    pub fn read_sub_path(&self, path: impl AsRef<Path>) -> Result<Vec<u8>> {
        let path = path.as_ref();
        for root in self.roots.iter() {
            let full_path = root.join(path);
            if full_path.exists() {
                return Ok(std::fs::read(full_path)?);
            }
        }

        for archive in self.archives.iter() {
            let mut archive = archive.write();
            if let Ok(mut sub_path) = archive.by_name(path.as_os_str().to_str().unwrap()) {
                let mut buf = Vec::new();
                sub_path.read_to_end(&mut buf)?;
                return Ok(buf);
            };
        }

        Err(anyhow!("Failed to read sub path: {:?}", path))
    }
}

pub struct LoadCtx<'w, 'f> {
    filesystem: &'f mut Filesystem,
    original_path: PathBuf,
    world: UnsafeWorldCell<'w>,
    resources_accessed: HashSet<TypeId>,
}

impl<'w, 'f> LoadCtx<'w, 'f> {
    pub fn new(filesystem: &'f mut Filesystem, world: &'w mut World) -> Self {
        Self {
            filesystem,
            original_path: PathBuf::new(),
            world: world.as_unsafe_world_cell(),
            resources_accessed: HashSet::new(),
        }
    }

    pub fn filesystem(&self) -> &Filesystem {
        self.filesystem
    }

    pub fn filesystem_mut(&mut self) -> &mut Filesystem {
        self.filesystem
    }

    pub fn original_path(&self) -> &Path {
        &self.original_path
    }

    pub fn read_original(&self) -> Result<Vec<u8>> {
        self.filesystem
            .read_sub_path(self.original_path.to_str().unwrap())
    }

    pub fn get_resource<T: Resource>(&mut self) -> Result<Res<'w, T>> {
        if self.resources_accessed.contains(&TypeId::of::<T>()) {
            return Err(anyhow!(
                "Resource of type {:?} already accessed",
                std::any::type_name::<T>()
            ));
        }
        let world = unsafe { self.world.world() };
        let resource = world.get_resource::<T>().ok_or_else(|| {
            anyhow!(
                "Resource of type {:?} not found",
                std::any::type_name::<T>()
            )
        })?;
        self.resources_accessed.insert(TypeId::of::<T>());
        Ok(resource)
    }

    pub fn get_resource_mut<T: Resource>(&mut self) -> Result<ResMut<'w, T>> {
        if self.resources_accessed.contains(&TypeId::of::<T>()) {
            return Err(anyhow!(
                "Resource of type {:?} already accessed",
                std::any::type_name::<T>()
            ));
        }
        let world = unsafe { self.world.world_mut() };
        let resource = world.get_resource_mut::<T>().ok_or_else(|| {
            anyhow!(
                "Resource of type {:?} not found",
                std::any::type_name::<T>()
            )
        })?;
        self.resources_accessed.insert(TypeId::of::<T>());
        Ok(resource)
    }

    pub fn drop_resource<T: Resource>(&mut self, _resource: Res<T>) {
        self.resources_accessed.remove(&TypeId::of::<T>());
    }

    pub fn drop_resource_mut<T: Resource>(&mut self, _resource: ResMut<T>) {
        self.resources_accessed.remove(&TypeId::of::<T>());
    }

    pub fn load_asset<T: Loadable, L: Loader<T>>(&mut self, path: impl AsRef<Path>) -> Result<T> {
        let old_path = self.original_path.clone();
        self.original_path = path.as_ref().to_path_buf();
        let world = unsafe { self.world.world_mut() };
        let loader = L::from_world(world);
        let asset = loader.load(self);
        self.original_path = old_path;
        asset
    }
}

pub trait Loader<T: Loadable>: Resource + FromWorld {
    fn load(&self, ctx: &mut LoadCtx<'_, '_>) -> Result<T>;
}

pub struct AssetLoader<'w, T: Loadable, L: Loader<T>> {
    loader: &'w mut L,
    world: &'w mut World,
    _marker: std::marker::PhantomData<fn() -> T>,
}

impl<'w, T: Loadable, L: Loader<T>> AssetLoader<'w, T, L> {
    pub fn load(self, path: impl AsRef<Path>) -> Result<T> {
        let path = path.as_ref();
        let mut filesystem = Filesystem::default();
        filesystem.add_root(path.parent().unwrap());
        let mut ctx = LoadCtx {
            filesystem: &mut filesystem,
            original_path: path.to_path_buf(),
            world: self.world.as_unsafe_world_cell(),
            resources_accessed: HashSet::new(),
        };
        self.loader.load(&mut ctx)
    }

    pub fn load_from_archive(
        self,
        archive_path: impl AsRef<Path>,
        path_within_archive: impl AsRef<Path>,
    ) -> Result<T> {
        let archive_path = archive_path.as_ref();
        let path_within_archive = path_within_archive.as_ref();
        let mut filesystem = Filesystem::default();
        filesystem.add_archive(archive_path)?;
        let mut ctx = LoadCtx {
            filesystem: &mut filesystem,
            original_path: path_within_archive.to_path_buf(),
            world: self.world.as_unsafe_world_cell(),
            resources_accessed: HashSet::new(),
        };
        self.loader.load(&mut ctx)
    }

    pub fn load_from_filesystem(
        self,
        filesystem: &mut Filesystem,
        path: impl AsRef<Path>,
    ) -> Result<T> {
        let path = path.as_ref();
        filesystem.add_root(path.parent().unwrap());
        let mut ctx = LoadCtx {
            filesystem,
            original_path: path.to_path_buf(),
            world: self.world.as_unsafe_world_cell(),
            resources_accessed: HashSet::new(),
        };
        self.loader.load(&mut ctx)
    }
}

unsafe impl<T: Loadable, L: Loader<T>> SystemParam for AssetLoader<'_, T, L> {
    type State = ();
    type Item<'w, 's> = AssetLoader<'w, T, L>;

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            ..Default::default()
        }
    }

    fn validate_access(_access: &SystemAccess) -> bool {
        true
    }

    fn init_state(_world: &mut World) -> Self::State {}

    unsafe fn fetch<'w, 's>(
        _: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        let loader = unsafe { world.get_resource_mut::<L>().unwrap() };
        AssetLoader {
            loader: loader.into_inner(),
            world: unsafe { world.world_mut() },
            _marker: std::marker::PhantomData,
        }
    }
}
