use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use weaver_ecs::{
    prelude::Resource,
    system::{SystemAccess, SystemParam, SystemParamItem},
    world::{FromWorld, UnsafeWorldCell, World},
};
use weaver_util::{
    lock::Lock,
    prelude::{anyhow, Result},
};
use zip::ZipArchive;

use crate::Asset;

pub trait Loadable {}

impl<T: Asset> Loadable for T {}
impl<T: Asset> Loadable for Vec<T> {}

/// Virtual filesystem created from one or more directories or archives.
#[derive(Default)]
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

    pub fn exists(&self, path: &str) -> bool {
        for root in self.roots.iter() {
            let full_path = root.join(path);
            if full_path.exists() {
                return true;
            }
        }

        for archive in self.archives.iter() {
            let mut archive = archive.write();
            if archive.by_name(path).is_ok() {
                return true;
            };
        }

        false
    }

    pub fn read_sub_path(&self, path: &str) -> Result<Vec<u8>> {
        for root in self.roots.iter() {
            let full_path = root.join(path);
            if full_path.exists() {
                return Ok(std::fs::read(full_path)?);
            }
        }

        for archive in self.archives.iter() {
            let mut archive = archive.write();
            if let Ok(mut sub_path) = archive.by_name(path) {
                let mut buf = Vec::new();
                sub_path.read_to_end(&mut buf)?;
                return Ok(buf);
            };
        }

        Err(anyhow!("Failed to read sub path '{}'", path))
    }
}

pub struct LoadCtx {
    filesystem: Filesystem,
    original_path: PathBuf,
}

impl LoadCtx {
    pub fn filesystem(&self) -> &Filesystem {
        &self.filesystem
    }

    pub fn original_path(&self) -> &Path {
        &self.original_path
    }

    pub fn read_original(&self) -> Result<Vec<u8>> {
        self.filesystem
            .read_sub_path(self.original_path.to_str().unwrap())
    }
}

pub trait LoadAsset<T: Loadable>: Resource + FromWorld {
    type Param: SystemParam + 'static;

    fn load(&self, param: SystemParamItem<Self::Param>, ctx: &mut LoadCtx) -> Result<T>;
}

pub struct AssetLoader<'w, 's, T: Loadable, L: LoadAsset<T>> {
    loader: &'w mut L,
    param: SystemParamItem<'w, 's, L::Param>,
    _marker: std::marker::PhantomData<T>,
}

impl<'w, 's, T: Loadable, L: LoadAsset<T>> AssetLoader<'w, 's, T, L> {
    pub fn load(self, path: impl AsRef<Path>) -> Result<T> {
        let path = path.as_ref();
        let mut filesystem = Filesystem::default();
        filesystem.add_root(path.parent().unwrap());
        let mut ctx = LoadCtx {
            filesystem,
            original_path: path.to_path_buf(),
        };
        self.loader.load(self.param, &mut ctx)
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
            filesystem,
            original_path: path_within_archive.to_path_buf(),
        };
        self.loader.load(self.param, &mut ctx)
    }

    pub fn load_from_filesystem(
        self,
        mut filesystem: Filesystem,
        path: impl AsRef<Path>,
    ) -> Result<T> {
        let path = path.as_ref();
        filesystem.add_root(path.parent().unwrap());
        let mut ctx = LoadCtx {
            filesystem,
            original_path: path.to_path_buf(),
        };
        self.loader.load(self.param, &mut ctx)
    }
}

unsafe impl<T: Loadable, L: LoadAsset<T>> SystemParam for AssetLoader<'_, '_, T, L> {
    type State = <L::Param as SystemParam>::State;
    type Item<'w, 's> = AssetLoader<'w, 's, T, L>;

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            ..Default::default()
        }
    }

    fn validate_access(_access: &SystemAccess) -> bool {
        true
    }

    fn init_state(world: &mut World) -> Self::State {
        <L::Param as SystemParam>::init_state(world)
    }

    unsafe fn fetch<'w, 's>(
        state: &'s mut Self::State,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Item<'w, 's> {
        let loader = unsafe { world.get_resource_mut::<L>().unwrap() };
        let param = <L::Param as SystemParam>::fetch(state, world);
        AssetLoader {
            loader: loader.into_inner(),
            param,
            _marker: std::marker::PhantomData,
        }
    }
}
