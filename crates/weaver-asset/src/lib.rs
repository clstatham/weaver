use std::{
    cell::UnsafeCell,
    fmt::Debug,
    ops::{Deref, DerefMut},
    path::Path,
    sync::atomic::AtomicUsize,
};

use weaver_app::{App, SubApp};
use weaver_ecs::{
    change::{ComponentTicks, Tick},
    prelude::{reflect_trait, Component, Reflect, Resource},
    storage::SparseSet,
    system::{SystemAccess, SystemParam, SystemParamItem},
    world::{FromWorld, UnsafeWorldCell, World},
};
use weaver_util::prelude::{anyhow, impl_downcast, DowncastSync, Error, Result};

pub mod prelude {
    pub use crate::{Asset, Assets, Handle, ReflectAsset, UntypedHandle};
    pub use weaver_asset_macros::Asset;
}

pub trait LoadAsset<T: Asset>: Resource + FromWorld {
    type Param: SystemParam + 'static;

    fn load(&mut self, param: &mut SystemParamItem<Self::Param>, path: &Path) -> Result<T>;
}

pub struct AssetLoader<'w, 's, T: Asset, L: LoadAsset<T>> {
    loader: &'w mut L,
    param: SystemParamItem<'w, 's, L::Param>,
    _marker: std::marker::PhantomData<T>,
}

impl<'w, 's, T: Asset, L: LoadAsset<T>> AssetLoader<'w, 's, T, L> {
    pub fn load(&mut self, path: impl AsRef<Path>) -> Result<T> {
        self.loader.load(&mut self.param, path.as_ref())
    }
}

unsafe impl<T: Asset, L: LoadAsset<T>> SystemParam for AssetLoader<'_, '_, T, L> {
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

#[reflect_trait]
pub trait Asset: DowncastSync {}
impl_downcast!(Asset);

impl Asset for () {}

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

pub struct AssetRef<'w, T: Asset> {
    asset: &'w T,
}

impl<'w, T: Asset> AssetRef<'w, T> {
    pub fn into_inner(self) -> &'w T {
        self.asset
    }
}

impl<'w, T: Asset> Deref for AssetRef<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.asset
    }
}

pub struct AssetMut<'w, T: Asset> {
    asset: &'w mut T,
}

impl<'w, T: Asset> AssetMut<'w, T> {
    pub fn into_inner(self) -> &'w mut T {
        self.asset
    }
}

impl<'w, T: Asset> Deref for AssetMut<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.asset
    }
}

impl<'w, T: Asset> DerefMut for AssetMut<'w, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.asset
    }
}

#[derive(Default, Resource)]
pub struct Assets<T: Asset> {
    next_handle_id: AtomicUsize,
    storage: SparseSet<UnsafeCell<T>>,
}

// SAFETY: Assets are Sync and we validate access to them before using them.
unsafe impl<T: Asset> Sync for Assets<T> {}

impl<T: Asset> Assets<T> {
    pub fn new() -> Self {
        Self {
            next_handle_id: AtomicUsize::new(0),
            storage: SparseSet::new(),
        }
    }

    pub fn insert(&mut self, asset: T) -> Handle<T> {
        let id = self
            .next_handle_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.storage.insert(
            id,
            UnsafeCell::new(asset),
            ComponentTicks::new(Tick::MAX), // todo: change detection for assets
        );

        Handle {
            id,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn get(&self, handle: Handle<T>) -> Option<AssetRef<T>> {
        self.storage.get(handle.id).map(|asset| {
            let asset = unsafe { &*asset.get() };
            AssetRef { asset }
        })
    }

    // todo: make this unsafe
    pub fn get_mut(&self, handle: Handle<T>) -> Option<AssetMut<T>> {
        self.storage.get(handle.id).map(|asset| {
            let asset = unsafe { &mut *asset.get() };
            AssetMut { asset }
        })
    }

    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        self.storage
            .remove(handle.id)
            .map(|asset| asset.0.into_inner())
    }
}

pub trait AddAsset {
    fn add_asset<T: Asset>(&mut self) -> &mut Self;
    fn add_asset_loader<T: Asset, L: LoadAsset<T>>(&mut self) -> &mut Self;
}

impl AddAsset for SubApp {
    fn add_asset<T: Asset>(&mut self) -> &mut Self {
        if !self.has_resource::<Assets<T>>() {
            self.insert_resource(Assets::<T>::new());
        }
        self
    }

    fn add_asset_loader<T: Asset, L: LoadAsset<T>>(&mut self) -> &mut Self {
        self.add_asset::<T>();
        let loader = L::from_world(self.world_mut());
        self.insert_resource(loader);
        self
    }
}

impl AddAsset for App {
    fn add_asset<T: Asset>(&mut self) -> &mut Self {
        self.main_app_mut().add_asset::<T>();
        self
    }

    fn add_asset_loader<T: Asset, L: LoadAsset<T>>(&mut self) -> &mut Self {
        self.main_app_mut().add_asset_loader::<T, L>();
        self
    }
}
