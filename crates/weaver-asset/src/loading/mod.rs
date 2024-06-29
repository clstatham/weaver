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
use weaver_util::{lock::Lock, prelude::Result};
use zip::ZipArchive;

use crate::Asset;

pub trait Loadable {}

impl<T: Asset> Loadable for T {}
impl<T: Asset> Loadable for Vec<T> {}

/// Base directory of where to load further associated assets from when loading an asset.
/// This will either be the directory the asset is located in or the root directory of an archive.
pub enum LoadRoot {
    Path(PathBuf),
    Archive(Lock<ZipArchive<BufReader<File>>>),
}

impl LoadRoot {
    pub fn read_sub_path(&self, path: &str) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        match self {
            LoadRoot::Path(root) => Some(File::open(root.join(path))?.read_to_end(&mut buf)?),
            LoadRoot::Archive(archive) => {
                Some(archive.write().by_name(path)?.read_to_end(&mut buf)?)
            }
        };
        Ok(buf)
    }
}

pub struct LoadCtx {
    root: LoadRoot,
    original_path: PathBuf,
}

impl LoadCtx {
    pub fn root(&self) -> &LoadRoot {
        &self.root
    }

    pub fn original_path(&self) -> &Path {
        &self.original_path
    }

    pub fn read_original(&self) -> Result<Vec<u8>> {
        self.root
            .read_sub_path(self.original_path.file_name().unwrap().to_str().unwrap())
    }
}

pub trait LoadAsset<T: Loadable>: Resource + FromWorld {
    type Param: SystemParam + 'static;

    fn load(&self, param: &mut SystemParamItem<Self::Param>, ctx: &mut LoadCtx) -> Result<T>;
}

pub struct AssetLoader<'w, 's, T: Loadable, L: LoadAsset<T>> {
    loader: &'w mut L,
    param: SystemParamItem<'w, 's, L::Param>,
    _marker: std::marker::PhantomData<T>,
}

impl<'w, 's, T: Loadable, L: LoadAsset<T>> AssetLoader<'w, 's, T, L> {
    pub fn load(&mut self, path: impl AsRef<Path>) -> Result<T> {
        let path = path.as_ref();
        let mut ctx = LoadCtx {
            root: LoadRoot::Path(path.parent().unwrap().to_path_buf()),
            original_path: path.to_path_buf(),
        };
        self.loader.load(&mut self.param, &mut ctx)
    }

    pub fn load_from_archive(
        &mut self,
        archive_path: impl AsRef<Path>,
        path_within_archive: impl AsRef<Path>,
    ) -> Result<T> {
        let archive_path = archive_path.as_ref();
        let path_within_archive = path_within_archive.as_ref();
        let archive = File::open(archive_path)?;
        let archive = BufReader::new(archive);
        let archive = ZipArchive::new(archive)?;
        let mut ctx = LoadCtx {
            root: LoadRoot::Archive(Lock::new(archive)),
            original_path: path_within_archive.to_path_buf(),
        };
        self.loader.load(&mut self.param, &mut ctx)
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
