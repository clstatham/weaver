use std::{
    cell::UnsafeCell,
    fmt::Debug,
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
};

use loading::{Loadable, Loader};
use weaver_app::{App, SubApp};
use weaver_ecs::prelude::{reflect_trait, Component, Resource};
use weaver_util::{
    define_atomic_id, {anyhow, impl_downcast, DowncastSync, Error, FxHashMap, Result},
};

pub mod loading;

pub mod prelude {
    pub use crate::{
        loading::{AssetLoader, Loader},
        Asset, Assets, Handle, ReflectAsset, UntypedHandle,
    };
    pub use weaver_asset_macros::Asset;
}

define_atomic_id!(AssetId);

#[reflect_trait]
pub trait Asset: DowncastSync {}
impl_downcast!(Asset);

impl Asset for () {}

#[derive(Component)]
pub struct Handle<T: Asset> {
    id: AssetId,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Asset> Handle<T> {
    pub const INVALID: Self = Self {
        id: AssetId::INVALID,
        _marker: std::marker::PhantomData,
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
            _marker: std::marker::PhantomData,
        }
    }

    pub const fn from_uuid(uuid: u128) -> Self {
        Self {
            id: AssetId::from_u64(uuid as u64),
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
                _marker: std::marker::PhantomData,
            })
        } else {
            Err(anyhow!("type mismatch"))
        }
    }
}

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

#[derive(Default, Resource)]
pub struct Assets<T: Asset> {
    storage: FxHashMap<AssetId, UnsafeCell<T>>,
}

// SAFETY: Assets are Sync and we validate access to them before using them.
unsafe impl<T: Asset> Sync for Assets<T> {}

impl<T: Asset> Assets<T> {
    pub fn new() -> Self {
        Self {
            storage: FxHashMap::default(),
        }
    }

    pub fn insert_manual(&mut self, asset: T, id: AssetId) -> Handle<T> {
        self.storage.insert(id, UnsafeCell::new(asset));

        Handle {
            id,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn insert(&mut self, asset: impl Into<T>) -> Handle<T> {
        let asset = asset.into();
        let id = AssetId::new();
        self.storage.insert(id, UnsafeCell::new(asset));

        Handle {
            id,
            _marker: std::marker::PhantomData,
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

pub trait AssetApp {
    fn add_asset<T: Asset>(&mut self) -> &mut Self;
    fn add_asset_loader<T: Asset + Loadable, L: Loader<T>>(&mut self) -> &mut Self;
    fn add_resource_loader<T: Resource + Loadable, L: Loader<T>>(&mut self) -> &mut Self;
}

impl AssetApp for SubApp {
    fn add_asset<T: Asset>(&mut self) -> &mut Self {
        if !self.has_resource::<Assets<T>>() {
            self.insert_resource(Assets::<T>::new());
        }
        self
    }

    fn add_asset_loader<T: Loadable + Asset, L: Loader<T>>(&mut self) -> &mut Self {
        self.add_asset::<T>();
        let loader = L::from_world(self.world_mut());
        self.insert_resource(loader);
        self
    }

    fn add_resource_loader<T: Resource + Loadable, L: Loader<T>>(&mut self) -> &mut Self {
        let loader = L::from_world(self.world_mut());
        self.insert_resource(loader);
        self
    }
}

impl AssetApp for App {
    fn add_asset<T: Asset>(&mut self) -> &mut Self {
        self.main_app_mut().add_asset::<T>();
        self
    }

    fn add_asset_loader<T: Loadable + Asset, L: Loader<T>>(&mut self) -> &mut Self {
        self.main_app_mut().add_asset_loader::<T, L>();
        self
    }

    fn add_resource_loader<T: Resource + Loadable, L: Loader<T>>(&mut self) -> &mut Self {
        self.main_app_mut().add_resource_loader::<T, L>();
        self
    }
}
