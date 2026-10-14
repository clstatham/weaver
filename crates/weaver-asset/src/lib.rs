//! Typed asset storage, handles and loading.
//!
//! Assets of type `T` live in an [`Assets<T>`] resource and are referred to by [`Handle<T>`], a
//! copyable id. A handle can exist before its asset does: asking a loader for something (see
//! [`AssetLoadQueue::enqueue`] or [`AssetCommands::lazy_load_asset`]) returns the handle at once
//! and the asset shows up in [`Assets<T>`] later, once the load has run.
//!
//! # Loading
//!
//! A loader is a type implementing [`LoadFrom<S>`] for some [`LoadSource`] `S` (a path, raw bytes,
//! another asset, ...). Registering it with [`AssetApp::add_asset_loader`] creates an
//! [`AssetLoadQueue`] for the (loader, source) pair and a system that drains it in the
//! [`AssetLoad`] stage. Loading is synchronous: a slow loader stalls the frame. Use
//! [`AssetApp::add_asset_load_dependency`] to make one loader's queue run before another's.
//!
//! # Events
//!
//! Changes to an [`Assets<T>`] are collected as [`AssetEvent<T>`]s (added, modified, removed) and a
//! successful load also sends an [`AssetLoaded<T>`]. Both are published in the [`AssetFlush`] stage
//! at the end of the frame, so readers see them on the next frame.
//!
//! # Setup
//!
//! ```
//! use weaver_app::prelude::*;
//! use weaver_asset::{AssetApp, prelude::*};
//!
//! #[derive(Asset)]
//! struct Text(String);
//!
//! let mut app = App::new();
//! app.add_asset::<Text>();
//! assert!(app.has_resource::<Assets<Text>>());
//! ```

use std::{
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
    commands::Commands,
    prelude::{IntoSystemConfig, Res, ResMut, SystemStage},
    world::{ConstructFromWorld, FromWorld},
};
use weaver_event::{EventQueue, EventTx};
use weaver_util::prelude::*;
use zip::ZipArchive;

/// The common asset types, including the [`Asset`] derive.
pub mod prelude {
    pub use crate::{
        Asset, AssetLoadQueue, Assets, DirectLoader, Filesystem, Handle, LoadFrom,
        PathAndFilesystem, UntypedHandle,
    };
    pub use weaver_asset_macros::Asset;
}

define_atomic_id!(
    /// Identifies one asset, independent of its type. [`Handle`] wraps it with the asset type.
    AssetId
);

/// Marker for types that can be stored in [`Assets`].
///
/// Use `#[derive(Asset)]` rather than writing the impl by hand. `()`, `Vec<T>` and `HashMap<K, T>`
/// of assets are assets too, which lets a loader return a bundle of them.
pub trait Asset: DowncastSync {
    /// Returns the type's name for log and panic messages.
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
impl<K, T, S> Asset for HashMap<K, T, S>
where
    K: Send + Sync + 'static,
    T: Asset,
    S: Send + Sync + 'static,
{
}

/// A typed reference to an asset in [`Assets<T>`].
///
/// A handle is just an id: copying it is free, and the asset it names may not exist yet (it could
/// still be queued for loading) or any more. Compare, hash and order by id only.
pub struct Handle<T: Asset> {
    id: AssetId,
    _marker: PhantomData<T>,
}

impl<T: Asset> Handle<T> {
    /// A handle that never refers to an asset.
    pub const INVALID: Self = Self {
        id: AssetId::INVALID,
        _marker: PhantomData,
    };

    /// Returns the underlying id.
    pub fn id(&self) -> AssetId {
        self.id
    }

    /// Erases the asset type, keeping its [`TypeId`](std::any::TypeId).
    pub fn into_untyped(self) -> UntypedHandle {
        self.into()
    }

    /// Wraps an id as a handle. Nothing checks that an asset of type `T` has that id.
    pub const fn from_raw(id: AssetId) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }

    /// Builds a handle with a fixed id derived from `uuid`, for well-known assets.
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
        *self
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

/// A [`Handle`] with the asset type stored at runtime instead of in the type.
///
/// Convert back with `Handle::<T>::try_from`, which fails if `T` isn't the original type.
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

/// Shared access to an asset. While it's alive, `Assets::get_mut` and `Assets::remove` on the same
/// asset panic.
pub struct AssetRef<T: Asset> {
    asset: OwnedRead<T>,
}

impl<T: Asset> Deref for AssetRef<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.asset
    }
}

/// Exclusive access to an asset. Getting one marks the asset as modified.
pub struct AssetMut<T: Asset> {
    asset: OwnedWrite<T>,
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

/// What happened to an asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetEventKind {
    /// The asset was inserted (or loaded) under a new id.
    Added,
    /// The asset was replaced, or borrowed mutably with [`Assets::get_mut`].
    Modified,
    /// The asset was removed.
    Removed,
}

/// Sent once per frame for every asset of type `T` that was added, modified or removed. Readers
/// (for example render-world extraction) use these to only process assets that changed.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct AssetEvent<T: Asset> {
    /// Which asset changed.
    pub id: AssetId,
    /// What happened to it.
    pub kind: AssetEventKind,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Asset> Clone for AssetEvent<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Asset> Copy for AssetEvent<T> {}

impl<T: Asset> AssetEvent<T> {
    /// Creates an event for asset `id`.
    pub fn new(id: AssetId, kind: AssetEventKind) -> Self {
        Self {
            id,
            kind,
            _marker: PhantomData,
        }
    }

    /// Returns a handle to the asset the event is about.
    pub fn handle(&self) -> Handle<T> {
        Handle::from_raw(self.id)
    }
}

/// Storage for all assets of type `T`.
///
/// Register it with [`AssetApp::add_asset`]. Every insert, replace, `get_mut` and remove queues an
/// [`AssetEvent`] that is published in [`AssetFlush`]. Assets sit behind their own locks, so an
/// [`AssetRef`] or [`AssetMut`] can be held without borrowing the collection.
pub struct Assets<T: Asset> {
    storage: FxHashMap<AssetId, SharedLock<T>>,
    /// Events since the last [`Assets::drain_events`], sent out once per frame by the
    /// `flush_asset_events` system.
    queued_events: Vec<AssetEvent<T>>,
}

impl<T: Asset> Default for Assets<T> {
    fn default() -> Self {
        Self {
            storage: FxHashMap::default(),
            queued_events: Vec::new(),
        }
    }
}

impl<T: Asset> Assets<T> {
    // TODO: handles aren't ref-counted, so assets are never freed unless removed by hand
    /// Creates an empty collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts an asset under a specific id, replacing (and reporting as modified) any asset
    /// already stored there.
    pub fn insert_manual(&mut self, asset: T, id: AssetId) -> Handle<T> {
        let kind = match self.storage.insert(id, SharedLock::new(asset)) {
            Some(_) => AssetEventKind::Modified,
            None => AssetEventKind::Added,
        };
        self.queued_events.push(AssetEvent::new(id, kind));

        Handle {
            id,
            _marker: PhantomData,
        }
    }

    /// Inserts an asset under a freshly minted id and returns its handle.
    pub fn insert(&mut self, asset: impl Into<T>) -> Handle<T> {
        self.insert_manual(asset.into(), AssetId::new())
    }

    /// Returns whether an asset exists for `handle`.
    pub fn contains(&self, handle: Handle<T>) -> bool {
        self.storage.contains_key(&handle.id)
    }

    /// Returns the number of stored assets.
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Returns whether no assets are stored.
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// Iterates over the ids of all stored assets, in no particular order.
    pub fn ids(&self) -> impl Iterator<Item = AssetId> + '_ {
        self.storage.keys().copied()
    }

    /// Borrows the asset for reading, or returns `None` if there's no such asset.
    ///
    /// # Panics
    ///
    /// Panics if the asset is currently borrowed through [`Assets::get_mut`].
    pub fn get(&self, handle: Handle<T>) -> Option<AssetRef<T>> {
        self.storage.get(&handle.id).map(|asset| {
            let asset = asset.try_read().unwrap_or_else(|| {
                panic!(
                    "{} {:?} is already mutably borrowed",
                    T::type_name(),
                    handle.id
                )
            });
            AssetRef { asset }
        })
    }

    /// Borrows the asset mutably and reports it as modified.
    ///
    /// The [`AssetEventKind::Modified`] event is queued as soon as the borrow is taken, whether or
    /// not the caller changes anything.
    ///
    /// # Panics
    ///
    /// Panics if the asset is currently borrowed.
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<AssetMut<T>> {
        let asset = self.storage.get(&handle.id)?;
        let asset = asset
            .try_write()
            .unwrap_or_else(|| panic!("{} {:?} is already borrowed", T::type_name(), handle.id));
        self.queued_events
            .push(AssetEvent::new(handle.id, AssetEventKind::Modified));
        Some(AssetMut { asset })
    }

    /// Removes the asset and returns it, or `None` if there's no such asset.
    ///
    /// # Panics
    ///
    /// Panics if the asset is currently borrowed.
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        let asset = self.storage.remove(&handle.id)?;
        self.queued_events
            .push(AssetEvent::new(handle.id, AssetEventKind::Removed));
        Some(
            asset
                .into_inner()
                .unwrap_or_else(|| panic!("{} {:?} is borrowed", T::type_name(), handle.id)),
        )
    }

    /// Takes the events queued since the last call.
    pub fn drain_events(&mut self) -> std::vec::Drain<'_, AssetEvent<T>> {
        self.queued_events.drain(..)
    }
}

/// Virtual filesystem created from one or more directories or archives.
///
/// Lookups try the roots in the order they were added, then the zip archives.
#[derive(Default)]
pub struct Filesystem {
    roots: Vec<PathBuf>,
    archives: Vec<Lock<ZipArchive<BufReader<File>>>>,
}

impl Filesystem {
    /// Creates a filesystem with no roots or archives.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder form of [`Filesystem::add_root`].
    pub fn with_root(mut self, root: impl AsRef<Path>) -> Self {
        self.add_root(root);
        self
    }

    /// Builder form of [`Filesystem::add_archive`].
    ///
    /// # Errors
    ///
    /// Returns an error if the archive can't be opened or isn't a valid zip file.
    pub fn with_archive(mut self, archive: impl AsRef<Path>) -> Result<Self> {
        self.add_archive(archive)?;
        Ok(self)
    }

    /// Adds a directory to search.
    pub fn add_root(&mut self, root: impl AsRef<Path>) {
        self.roots.push(root.as_ref().to_path_buf());
    }

    /// Adds a zip archive to search, after all directories.
    ///
    /// # Errors
    ///
    /// Returns an error if the archive can't be opened or isn't a valid zip file.
    pub fn add_archive(&mut self, archive: impl AsRef<Path>) -> Result<()> {
        let archive = File::open(archive)?;
        let archive = BufReader::new(archive);
        let archive = ZipArchive::new(archive)?;
        self.archives.push(Lock::new(archive));
        Ok(())
    }

    /// Lists the entries of a directory.
    ///
    /// The first root containing `dir_path` wins and its entries come back sorted. Otherwise every
    /// archive entry under `dir_path` is returned, nested ones included.
    ///
    /// # Errors
    ///
    /// Returns an error if no root or archive has the directory, or if reading it fails.
    ///
    /// # Panics
    ///
    /// Panics on an archive entry whose name isn't a safe relative path.
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

    /// Returns whether `path` exists in any root or archive.
    ///
    /// # Panics
    ///
    /// Panics if `path` isn't valid UTF-8 and there are archives to search.
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

    /// Reads the file at `path` from the first root or archive that has it.
    ///
    /// # Errors
    ///
    /// Returns an error if no root or archive has the file, or if reading it fails.
    ///
    /// # Panics
    ///
    /// Panics if `path` isn't valid UTF-8 and there are archives to search.
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

/// A load source that names a file inside a [`Filesystem`].
#[derive(Clone)]
pub struct PathAndFilesystem {
    /// Path relative to the filesystem's roots.
    pub path: PathBuf,
    /// The filesystem to read it from.
    pub fs: Arc<Filesystem>,
}

impl PathAndFilesystem {
    /// Creates a source for `path` in `fs`.
    pub fn new(path: impl AsRef<Path>, fs: Arc<Filesystem>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            fs,
        }
    }

    /// Reads the file's bytes.
    ///
    /// # Errors
    ///
    /// See [`Filesystem::read_sub_path`].
    pub fn read(&self) -> Result<Vec<u8>> {
        self.fs.read_sub_path(&self.path)
    }
}

impl From<(PathBuf, Arc<Filesystem>)> for PathAndFilesystem {
    fn from((path, fs): (PathBuf, Arc<Filesystem>)) -> Self {
        Self { path, fs }
    }
}

/// A type-erased asset, usable as a load source for [`DirectLoader`].
pub type BoxedAsset = Box<dyn Asset>;

/// Something a loader can be asked to load from: a path, raw bytes, another asset.
///
/// Every [`Asset`] is a `LoadSource`, so an asset can be handed straight to [`DirectLoader`].
pub trait LoadSource: Send + Sync + 'static {
    /// Returns a short description of the source for debug output, if it has one.
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

/// Turns a source `S` into an asset.
///
/// A loader is built from the world each time its queue is drained (it's a [`ConstructFromWorld`],
/// which every `Default` type is), so it can read configuration from resources. One loader type can
/// implement `LoadFrom` for several sources. Register it with [`AssetApp::add_asset_loader`].
///
/// `commands` is for side work: spawning entities, or asking for dependency assets with
/// [`AssetCommands::lazy_load_asset`]. Commands are applied after the loading system finishes, so
/// those dependencies load on a later pass.
///
/// # Examples
///
/// ```
/// use weaver_asset::prelude::*;
/// use weaver_ecs::commands::Commands;
/// use weaver_util::prelude::Result;
///
/// #[derive(Asset)]
/// struct Text(String);
///
/// #[derive(Default)]
/// struct TextLoader;
///
/// impl LoadFrom<Vec<u8>> for TextLoader {
///     type Asset = Text;
///
///     fn load(&self, source: Vec<u8>, _commands: &Commands) -> Result<Text> {
///         Ok(Text(String::from_utf8(source)?))
///     }
/// }
/// ```
pub trait LoadFrom<S: LoadSource>: ConstructFromWorld + Send + Sync + 'static {
    /// The type of asset this loader produces.
    type Asset: Asset;

    /// Loads one asset from `source`.
    ///
    /// # Errors
    ///
    /// Returns an error if the source can't be turned into an asset.
    fn load(&self, source: S, commands: &Commands) -> Result<Self::Asset>;
}

/// Loader that passes an asset through unchanged (or downcasts a [`BoxedAsset`] to `T`).
///
/// [`AssetApp::add_asset`] registers it for every asset type, which is what makes
/// [`AssetCommands::lazy_load_asset_direct`] work.
#[derive(Clone, Copy)]
pub struct DirectLoader<T: Asset>(PhantomData<T>);

impl<T: Asset> Default for DirectLoader<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Asset> LoadFrom<BoxedAsset> for DirectLoader<T> {
    type Asset = T;

    fn load(&self, source: BoxedAsset, _commands: &Commands) -> Result<T> {
        source
            .downcast()
            .map_err(|_| anyhow!("failed to downcast asset"))
            .map(|asset| *asset)
    }
}

impl<T: Asset> LoadFrom<T> for DirectLoader<T> {
    type Asset = T;
    fn load(&self, source: T, _commands: &Commands) -> Result<T> {
        Ok(source)
    }
}

/// A pending load: the handle that was handed out and the source to load it from.
pub struct AssetLoadRequest<T: Asset, S: LoadSource> {
    handle: Handle<T>,
    source: S,
}

impl<T: Asset, S: LoadSource> AssetLoadRequest<T, S> {
    /// Creates a request that will fill `handle` from `source`.
    pub fn new(handle: Handle<T>, source: S) -> Self {
        Self { handle, source }
    }

    /// Returns the handle the loaded asset will be stored under.
    pub fn handle(&self) -> Handle<T> {
        self.handle
    }

    /// Returns the source to load from.
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

/// Resource holding the pending loads for one (loader, source) pair.
///
/// It uses a lock internally, so it can be filled through a shared reference (`Res`). The
/// [`AssetLoad`] stage drains it every frame.
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
    /// Creates an empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues a load of `source` and returns the handle it will be stored under.
    pub fn enqueue(&self, source: impl Into<S>) -> Handle<L::Asset> {
        let handle = Handle::from_raw(AssetId::new());
        self.queue.write().push(AssetLoadRequest {
            handle,
            source: source.into(),
        });
        handle
    }

    /// Queues a load request that was built by hand.
    pub fn push(&self, request: AssetLoadRequest<L::Asset, S>) {
        self.queue.write().push(request);
    }

    /// Takes the most recently queued request, if any.
    pub fn pop(&self) -> Option<AssetLoadRequest<L::Asset, S>> {
        self.queue.write().pop()
    }

    /// Returns whether nothing is queued.
    pub fn is_empty(&self) -> bool {
        self.queue.read().is_empty()
    }

    /// Returns the number of queued requests.
    pub fn len(&self) -> usize {
        self.queue.read().len()
    }

    /// Drops all queued requests.
    pub fn clear(&self) {
        self.queue.write().clear();
    }
}

/// Extension methods on [`Commands`] for requesting asset loads from inside a system.
///
/// The handle is minted immediately; the request is put on the loader's [`AssetLoadQueue`] when the
/// commands are applied.
pub trait AssetCommands {
    /// Requests that loader `L` load `source`, returning the handle the result will get.
    ///
    /// The command panics when applied if `L` isn't registered for `S` (see
    /// [`AssetApp::add_asset_loader`]).
    fn lazy_load_asset<L: LoadFrom<S> + 'static, S: LoadSource>(
        &self,
        source: impl Into<S> + Send + Sync + 'static,
    ) -> Handle<L::Asset>;
    /// Requests that `asset` be inserted into [`Assets<T>`] through the [`DirectLoader`].
    fn lazy_load_asset_direct<T: Asset>(&self, asset: T) -> Handle<T>;
}

impl AssetCommands for Commands {
    fn lazy_load_asset<L: LoadFrom<S> + 'static, S: LoadSource>(
        &self,
        source: impl Into<S> + Send + Sync + 'static,
    ) -> Handle<L::Asset> {
        let handle = Handle::from_raw(AssetId::new());
        self.add(move |world| {
            world
                .get_resource::<AssetLoadQueue<L, S>>()
                .unwrap_or_else(|| panic!("No loader registered: {}", std::any::type_name::<L>()))
                .push(AssetLoadRequest::new(handle, source.into()));
        });
        handle
    }

    fn lazy_load_asset_direct<T: Asset>(&self, asset: T) -> Handle<T> {
        self.lazy_load_asset::<DirectLoader<T>, T>(asset)
    }
}

/// Sent when a loader has finished an asset and it is in [`Assets`].
///
/// A loaded asset also gets an [`AssetEventKind::Added`] [`AssetEvent`].
#[derive(Debug, Copy, PartialEq, Eq, Hash)]
pub struct AssetLoaded<T: Asset> {
    id: AssetId,
    _marker: PhantomData<T>,
}

impl<T: Asset> Clone for AssetLoaded<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _marker: PhantomData,
        }
    }
}

impl<T: Asset> AssetLoaded<T> {
    /// Creates an event for asset `id`.
    pub fn new(id: AssetId) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }

    /// Returns the id of the loaded asset.
    pub fn id(&self) -> AssetId {
        self.id
    }

    /// Returns a handle to the loaded asset.
    pub fn handle(&self) -> Handle<T> {
        Handle::from_raw(self.id)
    }
}

/// Resource recording which assets have been loaded, shared by all asset types.
///
/// The loading systems consult it to skip requests that were already satisfied; user code can
/// poll it to wait for a handle.
#[derive(Default)]
pub struct AssetLoadStatus {
    load_status: Lock<FxHashMap<AssetId, bool>>,
}

impl AssetLoadStatus {
    /// Creates an empty status table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks `handle` as loaded, for assets that were inserted without going through a loader.
    pub fn manually_set_loaded<T: Asset>(&self, handle: Handle<T>) {
        self.load_status.write().insert(handle.id, true);
    }

    /// Returns whether `handle` has been marked loaded.
    pub fn is_loaded<T: Asset>(&self, handle: Handle<T>) -> bool {
        self.load_status
            .read()
            .get(&handle.id)
            .copied()
            .unwrap_or(false)
    }
}

/// Stage where the loader queues are drained and assets are inserted.
///
/// Added after the app's other stages, so loads requested during a frame finish at the end of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
pub struct AssetLoad;

/// Runs after [`AssetLoad`]: sends the frame's [`AssetEvent`]s and [`AssetLoaded`] events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
pub struct AssetFlush;

/// Registers a double-buffered event queue for `T`, cleared in [`AssetFlush`].
fn add_asset_flush_event<E: weaver_event::Event>(app: &mut SubApp) {
    if !app.world().has_resource::<EventQueue<E>>() {
        app.add_event::<E>(AssetFlush);
    }
}

/// Sends the asset changes queued in `Assets<T>` this frame as [`AssetEvent`]s.
fn flush_asset_events<T: Asset>(mut assets: ResMut<Assets<T>>, mut events: EventTx<AssetEvent<T>>) {
    for event in assets.drain_events() {
        events.send(event);
    }
}

// Runs before the flush so this frame's events are still readable next frame.
fn clear_asset_events<T: Asset>(mut events: ResMut<EventQueue<AssetEvent<T>>>) {
    events.update();
}

/// Asset registration methods for [`App`] and [`SubApp`].
///
/// On an [`App`] they act on the main sub-app. All of them are idempotent.
pub trait AssetApp {
    /// Registers asset type `T`: its [`Assets<T>`] storage, its event queue, and the
    /// [`AssetLoad`] and [`AssetFlush`] stages if they're missing.
    ///
    /// Also registers a [`DirectLoader`] for `T`, so assets can be queued as-is.
    fn add_asset<T: Asset>(&mut self) -> &mut Self;
    /// Registers loader `L` for source type `S`: the [`AssetLoadQueue`] and the system that drains
    /// it in [`AssetLoad`].
    ///
    /// The loader's output type has to be registered with [`AssetApp::add_asset`] as well. The
    /// loading system reads and writes [`Assets<L::Asset>`](Assets), and without that resource it
    /// never runs.
    fn add_asset_loader<L: LoadFrom<S>, S: LoadSource>(&mut self) -> &mut Self;
    /// Makes loader `DL` (for source `DS`) run before loader `L` (for `S`) within [`AssetLoad`].
    ///
    /// This only orders the two systems. A load requested by `DL` while `L` runs is not waited for.
    fn add_asset_load_dependency<L: LoadFrom<S>, S: LoadSource, DL: LoadFrom<DS>, DS: LoadSource>(
        &mut self,
    ) -> &mut Self;
}

impl AssetApp for SubApp {
    fn add_asset<T: Asset>(&mut self) -> &mut Self {
        if !self.world().has_resource::<Assets<T>>() {
            self.world_mut().init_resource::<Assets<T>>();
            self.world_mut()
                .insert_resource(EventQueue::<AssetEvent<T>>::new());
            if !self.world().has_system_stage(AssetLoad) {
                self.world_mut().push_update_stage(AssetLoad);
            }
            if !self.world().has_system_stage(AssetFlush) {
                self.world_mut().push_update_stage(AssetFlush);
            }
            // clear first, so the events sent this frame stay readable through the next one
            self.world_mut()
                .add_system(clear_asset_events::<T>, AssetFlush);
            self.world_mut().add_system(
                flush_asset_events::<T>.after(clear_asset_events::<T>),
                AssetFlush,
            );
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

            if !self.world().has_system_stage(AssetLoad) {
                self.world_mut().push_update_stage(AssetLoad);
            }
            if !self.world().has_system_stage(AssetFlush) {
                self.world_mut().push_update_stage(AssetFlush);
            }
            add_asset_flush_event::<AssetLoaded<L::Asset>>(self);

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

// Drains the queue for one (loader, source) pair. Every request is loaded first and the results
// are inserted afterwards, so a loader never sees assets from its own batch in `Assets`.
fn load_all_assets<L: LoadFrom<S> + 'static, S: LoadSource>(
    loader: FromWorld<L>,
    load_queue: Res<AssetLoadQueue<L, S>>,
    mut assets: ResMut<Assets<L::Asset>>,
    mut load_events: ResMut<EventQueue<AssetLoaded<L::Asset>>>,
    load_status: Res<AssetLoadStatus>,
    commands: Commands,
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
        // TODO: failed loads are only logged and the request is dropped; the handle stays dangling
        let result = match loader.load(request.source, &commands) {
            Ok(asset) => Ok(asset),
            Err(e) => {
                log::error!("Failed to load asset: {}", e);
                Err(e)
            }
        };

        handles.push((request.handle, result));
    }

    for (handle, result) in handles {
        if let Ok(asset) = result {
            assets.insert_manual(asset, handle.id);
            load_status.manually_set_loaded(handle);
            load_events.send(AssetLoaded::new(handle.id));
        }
    }
}
