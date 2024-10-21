use std::{
    cell::UnsafeCell,
    fmt::Debug,
    fs::File,
    hash::{Hash, Hasher},
    io::{BufReader, Read},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::Arc,
};

use weaver_app::{App, SubApp};
use weaver_ecs::{
    prelude::{reflect_trait, Component, Res, ResMut, Resource, SystemStage},
    world::{FromWorld, World},
};
use weaver_event::{Event, Events};
use weaver_util::{
    anyhow, define_atomic_id, impl_downcast, DowncastSync, Error, FxHashMap, Lock, Result,
};
use zip::ZipArchive;

pub mod prelude {
    pub use crate::{
        Asset, AssetLoadQueue, AssetLoadQueues, Assets, DirectLoader, Filesystem, Handle, Loader,
        PathAndFilesystem, ReflectAsset, UntypedHandle,
    };
    pub use weaver_asset_macros::Asset;
}

define_atomic_id!(AssetId);

#[reflect_trait]
pub trait Asset: DowncastSync {}
impl_downcast!(Asset);

impl Asset for () {}
impl<T: Asset> Asset for Vec<T> {}

#[derive(Component, Resource)]
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

    pub const fn from_uuid(uuid: u128) -> Self {
        Self {
            id: AssetId::from_u64(uuid as u64),
            _marker: PhantomData,
        }
    }
}

impl<T: Asset> Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(&format!("Handle<{}>", std::any::type_name::<T>()))
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

#[derive(Clone, Copy)]
pub struct AssetRef<'w, T: Asset> {
    asset: &'w T,
}

impl<'w, T: Asset> AssetRef<'w, T> {
    #[inline]
    pub fn into_inner(self) -> &'w T {
        self.asset
    }
}

impl<'w, T: Asset> Deref for AssetRef<'w, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.asset
    }
}

pub struct AssetMut<'w, T: Asset> {
    asset: &'w mut T,
}

impl<'w, T: Asset> AssetMut<'w, T> {
    #[inline]
    pub fn into_inner(self) -> &'w mut T {
        self.asset
    }
}

impl<'w, T: Asset> Deref for AssetMut<'w, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.asset
    }
}

impl<'w, T: Asset> DerefMut for AssetMut<'w, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.asset
    }
}

#[derive(Resource)]
pub struct Assets<T: Asset> {
    storage: FxHashMap<AssetId, UnsafeCell<T>>,
}

// SAFETY: `Asset` implementors are Sync and we validate access to them before using them.
unsafe impl<T: Asset> Sync for Assets<T> {}

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
        self.storage.insert(id, UnsafeCell::new(asset));

        Handle {
            id,
            _marker: PhantomData,
        }
    }

    pub fn insert(&mut self, asset: impl Into<T>) -> Handle<T> {
        let asset = asset.into();
        let id = AssetId::new();
        self.storage.insert(id, UnsafeCell::new(asset));

        Handle {
            id,
            _marker: PhantomData,
        }
    }

    pub fn get(&self, handle: Handle<T>) -> Option<AssetRef<T>> {
        self.storage.get(&handle.id).map(|asset| {
            let asset = unsafe { &*asset.get() };
            AssetRef { asset }
        })
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<AssetMut<T>> {
        self.storage.get(&handle.id).map(|asset| {
            let asset = unsafe { &mut *asset.get() };
            AssetMut { asset }
        })
    }

    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        self.storage
            .remove(&handle.id)
            .map(|asset| asset.into_inner())
    }
}

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

pub trait LoadSource: Send + Sync + 'static {}

impl LoadSource for PathBuf {}
impl LoadSource for PathAndFilesystem {}
impl LoadSource for Vec<u8> {}
impl LoadSource for BoxedAsset {}
impl<T: Asset> LoadSource for T {}

pub trait Loader<T: Asset, S: LoadSource>: FromWorld + Send + Sync + 'static {
    fn load(&self, source: S, load_queues: &AssetLoadQueues<'_>) -> Result<T>;
}

#[derive(Clone, Copy)]
pub struct DirectLoader<T: Asset>(PhantomData<T>);

impl<T: Asset> Default for DirectLoader<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Asset> Loader<T, BoxedAsset> for DirectLoader<T> {
    fn load(&self, source: BoxedAsset, _load_queues: &AssetLoadQueues<'_>) -> Result<T> {
        source
            .downcast()
            .map_err(|_| anyhow!("failed to downcast asset"))
            .map(|asset| *asset)
    }
}

impl<T: Asset> Loader<T, T> for DirectLoader<T> {
    fn load(&self, source: T, _load_queues: &AssetLoadQueues<'_>) -> Result<T> {
        Ok(source)
    }
}

pub struct AssetLoadRequest<T: Asset, L: Loader<T, S>, S: LoadSource> {
    handle: Handle<T>,
    source: S,
    _marker: PhantomData<L>,
}

impl<T: Asset, L: Loader<T, S>, S: LoadSource> AssetLoadRequest<T, L, S> {
    pub fn new(handle: Handle<T>, source: S) -> Self {
        Self {
            handle,
            source,
            _marker: PhantomData,
        }
    }

    pub fn handle(&self) -> Handle<T> {
        self.handle
    }

    pub fn source(&self) -> &S {
        &self.source
    }
}

impl<T: Asset, L: Loader<T, S>, S: LoadSource> Debug for AssetLoadRequest<T, L, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetLoadRequest")
            .field("handle", &self.handle)
            .field("source", &std::any::type_name::<S>())
            .finish()
    }
}

#[derive(Resource)]
pub struct AssetLoadQueue<T: Asset, L: Loader<T, S>, S: LoadSource> {
    queue: Lock<Vec<AssetLoadRequest<T, L, S>>>,
}

impl<T: Asset, L: Loader<T, S>, S: LoadSource> Default for AssetLoadQueue<T, L, S> {
    fn default() -> Self {
        Self {
            queue: Lock::new(Vec::new()),
        }
    }
}

impl<T: Asset, L: Loader<T, S>, S: LoadSource> AssetLoadQueue<T, L, S> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&self, source: impl Into<S>) -> Handle<T> {
        let handle = Handle::from_raw(AssetId::new());
        self.queue.write().push(AssetLoadRequest {
            handle,
            source: source.into(),
            _marker: PhantomData,
        });
        handle
    }

    pub fn push(&self, request: AssetLoadRequest<T, L, S>) {
        self.queue.write().push(request);
    }

    pub fn pop(&self) -> Option<AssetLoadRequest<T, L, S>> {
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

#[derive(Clone, Copy)]
pub struct AssetLoadQueues<'w> {
    world: &'w World,
}

impl<'w> AssetLoadQueues<'w> {
    pub fn new(world: &'w World) -> Self {
        Self { world }
    }

    pub fn enqueue<T: Asset, L: Loader<T, S>, S: LoadSource>(
        &self,
        source: impl Into<S>,
    ) -> Option<Handle<T>> {
        self.world
            .get_resource::<AssetLoadQueue<T, L, S>>()
            .map(|load_queue| load_queue.enqueue(source))
    }

    pub fn enqueue_direct<T: Asset>(&self, source: impl Into<T>) -> Option<Handle<T>> {
        self.enqueue::<T, DirectLoader<T>, T>(source.into())
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

#[derive(Resource, Default)]
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

pub struct AssetLoad;
impl SystemStage for AssetLoad {}

pub trait AssetApp {
    fn add_asset<T: Asset>(&mut self) -> &mut Self;
    fn add_asset_loader<T: Asset, L: Loader<T, S>, S: LoadSource>(&mut self) -> &mut Self;
    fn add_asset_load_dependency<
        T: Asset,
        L: Loader<T, S>,
        S: LoadSource,
        D: Asset,
        DL: Loader<D, DS>,
        DS: LoadSource,
    >(
        &mut self,
    ) -> &mut Self;
}

impl AssetApp for SubApp {
    fn add_asset<T: Asset>(&mut self) -> &mut Self {
        if !self.has_resource::<Assets<T>>() {
            self.init_resource::<Assets<T>>();
        }
        if !self.has_resource::<AssetLoadStatus>() {
            self.init_resource::<AssetLoadStatus>();
        }
        if !self.has_resource::<AssetLoadQueue<T, DirectLoader<T>, BoxedAsset>>() {
            self.add_asset_loader::<T, DirectLoader<T>, BoxedAsset>();
        }
        if !self.has_resource::<AssetLoadQueue<T, DirectLoader<T>, T>>() {
            self.add_asset_loader::<T, DirectLoader<T>, T>();
        }
        self
    }

    fn add_asset_loader<T: Asset, L: Loader<T, S>, S: LoadSource>(&mut self) -> &mut Self {
        if !self.has_resource::<AssetLoadQueue<T, L, S>>() {
            self.init_resource::<AssetLoadQueue<T, L, S>>();

            self.init_resource::<Events<AssetLoaded<T>>>();

            if !self.has_system_stage::<AssetLoad>() {
                self.push_update_stage::<AssetLoad>();
            }

            self.add_system(load_all_assets::<T, L, S>, AssetLoad);
        }
        self
    }

    fn add_asset_load_dependency<
        T: Asset,
        L: Loader<T, S>,
        S: LoadSource,
        D: Asset,
        DL: Loader<D, DS>,
        DS: LoadSource,
    >(
        &mut self,
    ) -> &mut Self {
        self.add_system_dependency(
            load_all_assets::<T, L, S>,
            load_all_assets::<D, DL, DS>,
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

    fn add_asset_loader<T: Asset, L: Loader<T, S>, S: LoadSource>(&mut self) -> &mut Self {
        self.main_app_mut().add_asset_loader::<T, L, S>();
        self
    }

    fn add_asset_load_dependency<
        T: Asset,
        L: Loader<T, S>,
        S: LoadSource,
        D: Asset,
        DL: Loader<D, DS>,
        DS: LoadSource,
    >(
        &mut self,
    ) -> &mut Self {
        self.main_app_mut()
            .add_asset_load_dependency::<T, L, S, D, DL, DS>();
        self
    }
}

fn load_all_assets<T: Asset, L: Loader<T, S>, S: LoadSource>(
    world: &mut World,
    load_queue: Res<AssetLoadQueue<T, L, S>>,
    mut assets: ResMut<Assets<T>>,
    load_events: Res<Events<AssetLoaded<T>>>,
    load_status: Res<AssetLoadStatus>,
) {
    if load_queue.is_empty() {
        return;
    }
    log::debug!("Loading all assets of type: {}", std::any::type_name::<T>());

    let loader = Arc::new(L::from_world(world));

    let n = load_queue.len();
    let load_queues = AssetLoadQueues::new(world);

    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(n);

        for _ in 0..n {
            let request = load_queue.pop().unwrap();
            if assets.get(request.handle).is_some() || load_status.is_loaded(request.handle) {
                continue;
            }

            log::trace!("Loading asset: {:?}", request);
            let loader = loader.clone();
            let join_handle =
                scope.spawn(move || match loader.load(request.source, &load_queues) {
                    Ok(asset) => Some(asset),
                    Err(err) => {
                        log::error!("Failed to load asset: {}", err);
                        None
                    }
                });

            handles.push((request.handle, join_handle));
        }

        for (handle, join_handle) in handles {
            if let Some(asset) = join_handle.join().unwrap() {
                assets.insert_manual(asset, handle.id);
                load_status.manually_set_loaded(handle);
                load_events.send(AssetLoaded::new(handle.id));
            }
        }
    });
}
