use std::{path::Path, sync::atomic::AtomicUsize};

use weaver_app::{plugin::Plugin, App};
use weaver_ecs::{
    prelude::{Component, Resource},
    storage::SparseSet,
};
use weaver_util::prelude::{anyhow, impl_downcast, DowncastSync, Error, Result};

pub mod prelude {
    pub use crate::{Asset, AssetPlugin, Assets, Handle};
    pub use weaver_asset_macros::Asset;
}

pub trait Asset: DowncastSync {
    fn load(assets: &mut Assets, path: &std::path::Path) -> Result<Self>
    where
        Self: Sized;
}
impl_downcast!(Asset);

#[derive(Debug, Component)]
pub struct Handle<T: Asset> {
    id: usize,
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

#[derive(Default, Resource)]
pub struct Assets {
    next_handle_id: AtomicUsize,
    storage: SparseSet<Box<dyn Asset>>,
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
        self.storage.insert(id, Box::new(asset));

        Handle {
            id,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn get<T: Asset>(&self, handle: Handle<T>) -> Option<&T> {
        self.storage
            .get(handle.id)
            .and_then(|asset| (**asset).downcast_ref())
    }

    pub fn get_mut<T: Asset>(&mut self, handle: Handle<T>) -> Option<&mut T> {
        self.storage
            .get_mut(handle.id)
            .and_then(|asset| (**asset).downcast_mut())
    }

    pub fn remove<T: Asset>(&mut self, handle: Handle<T>) -> Option<T> {
        self.storage
            .remove(handle.id)
            .and_then(|asset| asset.downcast().ok())
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
