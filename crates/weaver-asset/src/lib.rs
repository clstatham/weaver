use std::{
    fmt::Debug,
    fs::File,
    future::Future,
    hash::{Hash, Hasher},
    io::{BufReader, Read},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::Arc,
};

use weaver_app::{App, SubApp};
use weaver_ecs::{
    loan::{Loan, LoanMut, LoanStorage},
    prelude::{Commands, Res, ResMut, SystemStage},
    world::{ConstructFromWorld, FromWorld},
};
use weaver_event::{Event, Events};
use weaver_task::usages::GlobalTaskPool;
use weaver_util::prelude::*;
use zip::ZipArchive;

pub mod prelude {
    pub use crate::{
        Asset, AssetLoadQueue, Assets, DirectLoader, Filesystem, Handle, LoadFrom,
        PathAndFilesystem, UntypedHandle,
    };
    pub use weaver_asset_macros::Asset;
}

define_atomic_id!(AssetId);

pub trait Asset: DowncastSync {
    fn type_name() -> &'static str
    where
        Self: Sized,
    {
        std::any::type_name::<Self>()
    }
}
impl_downcast!(Asset);

impl Asset for () {}
impl<T: Asset> Asset for Vec<T> {}

pub struct Handle<T: Asset> {
    id: AssetId,
    _marker: PhantomData<T>,
}

impl<T: Asset> Handle<T> {
    pub const INVALID: Self = Self {
        id: AssetId::INVALID,
        _marker: PhantomData,
    };

    pub fn id(&self) -> AssetId {
        self.id
    }

    pub fn into_untyped(self) -> UntypedHandle {
        self.into()
    }

    pub const fn from_raw(id: AssetId) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }

    pub const fn from_u128(uuid: u128) -> Self {
        Self {
            id: AssetId::from_u128(uuid),
            _marker: PhantomData,
        }
    }
}

impl<T: Asset> Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(&format!("Handle<{}>", T::type_name()))
            .field(&self.id)
            .finish()
    }
}

impl<T: Asset> Clone for Handle<T> {
    #[allow(clippy::non_canonical_clone_impl)]
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _marker: PhantomData,
        }
    }
}

impl<T: Asset> Copy for Handle<T> {}

impl<T: Asset> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: Asset> Eq for Handle<T> {}

impl<T: Asset> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl<T: Asset> PartialOrd for Handle<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl<T: Asset> Ord for Handle<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UntypedHandle {
    id: AssetId,
    type_id: std::any::TypeId,
}

impl<T: Asset> From<Handle<T>> for UntypedHandle {
    fn from(handle: Handle<T>) -> Self {
        Self {
            id: handle.id,
            type_id: std::any::TypeId::of::<T>(),
        }
    }
}

impl<T: Asset> TryFrom<UntypedHandle> for Handle<T> {
    type Error = Error;

    fn try_from(untyped_handle: UntypedHandle) -> Result<Self, Self::Error> {
        if untyped_handle.type_id == std::any::TypeId::of::<T>() {
            Ok(Self {
                id: untyped_handle.id,
                _marker: PhantomData,
            })
        } else {
            Err(anyhow!("type mismatch"))
        }
    }
}

#[derive(Clone)]
pub struct AssetRef<T: Asset> {
    asset: Loan<T>,
}

impl<T: Asset> Deref for AssetRef<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.asset
    }
}

pub struct AssetMut<T: Asset> {
    asset: LoanMut<T>,
}

impl<T: Asset> Deref for AssetMut<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.asset
    }
}

impl<T: Asset> DerefMut for AssetMut<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.asset
    }
}

pub struct Assets<T: Asset> {
    storage: FxHashMap<AssetId, LoanStorage<T>>,
}

impl<T: Asset> Default for Assets<T> {
    fn default() -> Self {
        Self {
            storage: FxHashMap::default(),
        }
    }
}

impl<T: Asset> Assets<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_manual(&mut self, asset: T, id: AssetId) -> Handle<T> {
        self.storage.insert(id, LoanStorage::new(asset));

        Handle {
            id,
            _marker: PhantomData,
        }
    }

    pub fn insert(&mut self, asset: impl Into<T>) -> Handle<T> {
        let asset = asset.into();
        let id = AssetId::new();
        self.storage.insert(id, LoanStorage::new(asset));

        Handle {
            id,
            _marker: PhantomData,
        }
    }

    pub fn get(&mut self, handle: Handle<T>) -> Option<AssetRef<T>> {
        self.storage.get_mut(&handle.id).map(|asset| {
            let asset = asset.loan().expect("asset is already borrowed");
            AssetRef { asset }
        })
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<AssetMut<T>> {
        self.storage.get_mut(&handle.id).map(|asset| {
            let asset = asset.loan_mut().expect("asset is already borrowed");
            AssetMut { asset }
        })
    }

    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        self.storage.remove(&handle.id).map(|asset| {
            asset
                .into_owned()
                .unwrap_or_else(|_| panic!("asset is borrowed"))
        })
    }
}

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

#[derive(Clone)]
pub struct PathAndFilesystem {
    pub path: PathBuf,
    pub fs: Arc<Filesystem>,
}

impl PathAndFilesystem {
    pub fn new(path: impl AsRef<Path>, fs: Arc<Filesystem>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            fs,
        }
    }

    pub fn read(&self) -> Result<Vec<u8>> {
        self.fs.read_sub_path(&self.path)
    }
}

impl From<(PathBuf, Arc<Filesystem>)> for PathAndFilesystem {
    fn from((path, fs): (PathBuf, Arc<Filesystem>)) -> Self {
        Self { path, fs }
    }
}

pub type BoxedAsset = Box<dyn Asset>;

pub trait LoadSource: Send + Sync + 'static {
    fn as_str(&self) -> Option<&str> {
        None
    }
}

impl LoadSource for PathBuf {
    fn as_str(&self) -> Option<&str> {
        self.to_str()
    }
}
impl LoadSource for PathAndFilesystem {
    fn as_str(&self) -> Option<&str> {
        self.path.to_str()
    }
}
impl LoadSource for Vec<u8> {}
impl LoadSource for BoxedAsset {}
impl<T: Asset> LoadSource for T {}

pub trait LoadFrom<S: LoadSource>: ConstructFromWorld + Send + Sync + 'static {
    type Asset: Asset;

    fn load(
        &self,
        source: S,
        commands: &Commands,
    ) -> impl Future<Output = Result<Self::Asset>> + Send + Sync;
}

#[derive(Clone, Copy)]
pub struct DirectLoader<T: Asset>(PhantomData<T>);

impl<T: Asset> Default for DirectLoader<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Asset> LoadFrom<BoxedAsset> for DirectLoader<T> {
    type Asset = T;

    async fn load(&self, source: BoxedAsset, _commands: &Commands) -> Result<T> {
        source
            .downcast()
            .map_err(|_| anyhow!("failed to downcast asset"))
            .map(|asset| *asset)
    }
}

impl<T: Asset> LoadFrom<T> for DirectLoader<T> {
    type Asset = T;
    async fn load(&self, source: T, _commands: &Commands) -> Result<T> {
        Ok(source)
    }
}

pub struct AssetLoadRequest<T: Asset, S: LoadSource> {
    handle: Handle<T>,
    source: S,
}

impl<T: Asset, S: LoadSource> AssetLoadRequest<T, S> {
    pub fn new(handle: Handle<T>, source: S) -> Self {
        Self { handle, source }
    }

    pub fn handle(&self) -> Handle<T> {
        self.handle
    }

    pub fn source(&self) -> &S {
        &self.source
    }
}

impl<T: Asset, S: LoadSource> Debug for AssetLoadRequest<T, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let source = if let Some(s) = self.source.as_str() {
            s
        } else {
            T::type_name()
        };
        f.debug_struct("AssetLoadRequest")
            .field("handle", &self.handle)
            .field("source", &source)
            .finish()
    }
}

pub struct AssetLoadQueue<L: LoadFrom<S>, S: LoadSource> {
    queue: Lock<Vec<AssetLoadRequest<L::Asset, S>>>,
    _marker: PhantomData<L>,
}

impl<L: LoadFrom<S>, S: LoadSource> Default for AssetLoadQueue<L, S> {
    fn default() -> Self {
        Self {
            queue: Lock::new(Vec::new()),
            _marker: PhantomData,
        }
    }
}

impl<L: LoadFrom<S>, S: LoadSource> AssetLoadQueue<L, S> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&self, source: impl Into<S>) -> Handle<L::Asset> {
        let handle = Handle::from_raw(AssetId::new());
        self.queue.write().push(AssetLoadRequest {
            handle,
            source: source.into(),
        });
        handle
    }

    pub fn push(&self, request: AssetLoadRequest<L::Asset, S>) {
        self.queue.write().push(request);
    }

    pub fn pop(&self) -> Option<AssetLoadRequest<L::Asset, S>> {
        self.queue.write().pop()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.read().is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.read().len()
    }

    pub fn clear(&self) {
        self.queue.write().clear();
    }
}

pub trait AssetCommands {
    fn lazy_load_asset<L: LoadFrom<S> + 'static, S: LoadSource>(
        &self,
        source: impl Into<S> + Send + Sync + 'static,
    ) -> Handle<L::Asset>;
    fn lazy_load_asset_direct<T: Asset>(&self, asset: T) -> Handle<T>;
}

impl AssetCommands for Commands {
    fn lazy_load_asset<L: LoadFrom<S> + 'static, S: LoadSource>(
        &self,
        source: impl Into<S> + Send + Sync + 'static,
    ) -> Handle<L::Asset> {
        self.run(|world| {
            world
                .get_resource::<AssetLoadQueue<L, S>>()
                .map(|load_queue| load_queue.enqueue(source.into()))
                .unwrap()
        })
    }

    fn lazy_load_asset_direct<T: Asset>(&self, asset: T) -> Handle<T> {
        self.lazy_load_asset::<DirectLoader<T>, T>(asset)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetLoaded<T: Asset> {
    id: AssetId,
    _marker: PhantomData<T>,
}
impl<T: Asset> Event for AssetLoaded<T> {}
impl<T: Asset> AssetLoaded<T> {
    pub fn new(id: AssetId) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }

    pub fn id(&self) -> AssetId {
        self.id
    }

    pub fn handle(&self) -> Handle<T> {
        Handle::from_raw(self.id)
    }
}

#[derive(Default)]
pub struct AssetLoadStatus {
    load_status: Lock<FxHashMap<AssetId, bool>>,
}

impl AssetLoadStatus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn manually_set_loaded<T: Asset>(&self, handle: Handle<T>) {
        self.load_status.write().insert(handle.id, true);
    }

    pub fn is_loaded<T: Asset>(&self, handle: Handle<T>) -> bool {
        self.load_status
            .read()
            .get(&handle.id)
            .copied()
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
pub struct AssetLoad;

pub trait AssetApp {
    fn add_asset<T: Asset>(&mut self) -> &mut Self;
    fn add_asset_loader<L: LoadFrom<S>, S: LoadSource>(&mut self) -> &mut Self;
    fn add_asset_load_dependency<L: LoadFrom<S>, S: LoadSource, DL: LoadFrom<DS>, DS: LoadSource>(
        &mut self,
    ) -> &mut Self;
}

impl AssetApp for SubApp {
    fn add_asset<T: Asset>(&mut self) -> &mut Self {
        if !self.world().has_resource::<Assets<T>>() {
            self.world_mut().init_resource::<Assets<T>>();
        }
        if !self.world().has_resource::<AssetLoadStatus>() {
            self.world_mut().init_resource::<AssetLoadStatus>();
        }
        if !self
            .world()
            .has_resource::<AssetLoadQueue<DirectLoader<T>, BoxedAsset>>()
        {
            self.add_asset_loader::<DirectLoader<T>, BoxedAsset>();
        }
        if !self
            .world()
            .has_resource::<AssetLoadQueue<DirectLoader<T>, T>>()
        {
            self.add_asset_loader::<DirectLoader<T>, T>();
        }
        self
    }

    fn add_asset_loader<L: LoadFrom<S>, S: LoadSource>(&mut self) -> &mut Self {
        if !self.world().has_resource::<AssetLoadQueue<L, S>>() {
            self.world_mut().init_resource::<AssetLoadQueue<L, S>>();

            self.world_mut()
                .init_resource::<Events<AssetLoaded<L::Asset>>>();

            if !self.world().has_system_stage(AssetLoad) {
                self.world_mut().push_update_stage(AssetLoad);
            }

            self.world_mut()
                .add_system(load_all_assets::<L, S>, AssetLoad);
        }
        self
    }

    fn add_asset_load_dependency<
        L: LoadFrom<S>,
        S: LoadSource,
        DL: LoadFrom<DS>,
        DS: LoadSource,
    >(
        &mut self,
    ) -> &mut Self {
        self.world_mut().order_systems(
            load_all_assets::<DL, DS>,
            load_all_assets::<L, S>,
            AssetLoad,
        );
        self
    }
}

impl AssetApp for App {
    fn add_asset<T: Asset>(&mut self) -> &mut Self {
        self.main_app_mut().add_asset::<T>();
        self
    }

    fn add_asset_loader<L: LoadFrom<S>, S: LoadSource>(&mut self) -> &mut Self {
        self.main_app_mut().add_asset_loader::<L, S>();
        self
    }

    fn add_asset_load_dependency<
        L: LoadFrom<S>,
        S: LoadSource,
        DL: LoadFrom<DS>,
        DS: LoadSource,
    >(
        &mut self,
    ) -> &mut Self {
        self.main_app_mut()
            .add_asset_load_dependency::<L, S, DL, DS>();
        self
    }
}

async fn load_all_assets<L: LoadFrom<S> + 'static, S: LoadSource>(
    commands: Commands,
    loader: FromWorld<L>,
    load_queue: Res<AssetLoadQueue<L, S>>,
    mut assets: ResMut<Assets<L::Asset>>,
    load_events: Res<Events<AssetLoaded<L::Asset>>>,
    load_status: Res<AssetLoadStatus>,
) {
    if load_queue.is_empty() {
        return;
    }
    log::debug!("Loading all assets of type: {}", L::Asset::type_name());

    let n = load_queue.len();

    let mut handles = Vec::with_capacity(n);

    let loader = Arc::new(loader);

    for request in load_queue.queue.write().drain(..) {
        if assets.get(request.handle).is_some() || load_status.is_loaded(request.handle) {
            continue;
        }
        let loader = loader.clone();
        let commands = commands.clone();
        let task = GlobalTaskPool::get().spawn(async move {
            match loader.load(request.source, &commands).await {
                Ok(asset) => Ok(asset),
                Err(e) => {
                    log::error!("Failed to load asset: {}", e);
                    Err(e)
                }
            }
        });

        handles.push((request.handle, task));
    }

    for (handle, result) in handles {
        if let Ok(asset) = result.await {
            assets.insert_manual(asset, handle.id);
            load_status.manually_set_loaded(handle);
            load_events.send(AssetLoaded::new(handle.id));
        }
    }
}
