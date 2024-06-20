use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
    path::Path,
    sync::atomic::AtomicUsize,
};

use weaver_app::{plugin::Plugin, App};
use weaver_ecs::{
    prelude::{reflect_trait, Component, Reflect, Resource},
    storage::SparseSet,
};
use weaver_util::{
    lock::{ArcRead, ArcWrite, SharedLock},
    prelude::{anyhow, impl_downcast, DowncastSync, Error, Result},
};

pub mod prelude {
    pub use crate::{Asset, AssetPlugin, Assets, Handle, ReflectAsset, UntypedHandle};
    pub use weaver_asset_macros::Asset;
}

#[reflect_trait]
pub trait Asset: DowncastSync {
    fn load(assets: &mut Assets, path: &std::path::Path) -> Result<Self>
    where
        Self: Sized;
}
impl_downcast!(Asset);

#[derive(Component, Reflect)]
pub struct Handle<T: Asset> {
    id: usize,
    #[reflect(ignore)]
    _marker: std::marker::PhantomData<T>,
}

impl<T: Asset> Handle<T> {
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn into_untyped(self) -> UntypedHandle {
        self.into()
    }

    pub fn from_raw(id: usize) -> Self {
        Self {
            id,
            _marker: std::marker::PhantomData,
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
            _marker: std::marker::PhantomData,
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

impl<T: Asset> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
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
    id: usize,
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
                _marker: std::marker::PhantomData,
            })
        } else {
            Err(anyhow!("type mismatch"))
        }
    }
}

pub struct AssetRef<T: Asset> {
    asset: ArcRead<Box<dyn Asset>>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Asset> Deref for AssetRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        (**self.asset).downcast_ref().expect("invalid asset")
    }
}

pub struct AssetMut<T: Asset> {
    asset: ArcWrite<Box<dyn Asset>>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Asset> Deref for AssetMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        (**self.asset).downcast_ref().expect("invalid asset")
    }
}

impl<T: Asset> DerefMut for AssetMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        (**self.asset).downcast_mut().expect("invalid asset")
    }
}

#[derive(Default, Resource)]
pub struct Assets {
    next_handle_id: AtomicUsize,
    storage: SparseSet<SharedLock<Box<dyn Asset>>>,
}

impl Assets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load<T: Asset>(&mut self, path: impl AsRef<Path>) -> Result<Handle<T>> {
        let asset = T::load(self, path.as_ref())?;
        Ok(self.insert(asset))
    }

    pub fn insert<T: Asset>(&mut self, asset: T) -> Handle<T> {
        let id = self
            .next_handle_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.storage.insert(id, SharedLock::new(Box::new(asset)));

        Handle {
            id,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn get<T: Asset>(&self, handle: Handle<T>) -> Option<AssetRef<T>> {
        self.storage.get(handle.id).and_then(|asset| {
            let asset = asset.read_arc();
            asset.is::<T>().then(|| AssetRef {
                asset,
                _marker: std::marker::PhantomData,
            })
        })
    }

    pub fn get_mut<T: Asset>(&self, handle: Handle<T>) -> Option<AssetMut<T>> {
        self.storage.get(handle.id).and_then(|asset| {
            let asset = asset.write_arc();
            asset.is::<T>().then(|| AssetMut {
                asset,
                _marker: std::marker::PhantomData,
            })
        })
    }

    pub fn remove<T: Asset>(&mut self, handle: Handle<T>) -> Option<T> {
        self.storage
            .remove(handle.id)
            .and_then(|asset| SharedLock::into_inner(asset).unwrap().downcast().ok())
            .map(|asset| *asset)
    }
}

pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.insert_resource(Assets::new());
        Ok(())
    }
}
