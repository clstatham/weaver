use std::{
    any::Any,
    collections::HashMap,
    path::{Path, PathBuf},
    sync::atomic::AtomicUsize,
};

use loader::AssetLoader;
use weaver_app::{plugin::Plugin, App};
use weaver_ecs::storage::SparseSet;
use weaver_util::prelude::{anyhow, Error, Result};

pub mod loader;

pub mod prelude {
    pub use crate::{loader::AssetLoader, Asset, AssetPlugin, Assets, Handle};
}

pub trait Asset: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn as_any_box(self: Box<Self>) -> Box<dyn Any>;
}

impl<T: 'static> Asset for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any_box(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

#[derive(Debug)]
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

#[derive(Default)]
pub struct Assets {
    next_handle_id: AtomicUsize,
    storage: SparseSet<Box<dyn Asset>>,
    paths: HashMap<PathBuf, UntypedHandle>,
}

impl Assets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T: Any>(&mut self, asset: T, path: Option<&Path>) -> Handle<T> {
        let id = self
            .next_handle_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.storage.insert(id, Box::new(asset));
        if let Some(path) = path {
            self.paths.insert(
                path.into(),
                UntypedHandle {
                    id,
                    type_id: std::any::TypeId::of::<T>(),
                },
            );
        }

        Handle {
            id,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn get<T: Any>(&self, handle: Handle<T>) -> Option<&T> {
        self.storage
            .get(handle.id)
            .and_then(|asset| (**asset).as_any().downcast_ref())
    }

    pub fn find_by_path(&self, path: impl AsRef<Path>) -> Option<UntypedHandle> {
        self.paths.get(&PathBuf::from(path.as_ref())).copied()
    }

    pub fn get_mut<T: Any>(&mut self, handle: Handle<T>) -> Option<&mut T> {
        self.storage
            .get_mut(handle.id)
            .and_then(|asset| (**asset).as_any_mut().downcast_mut())
    }

    pub fn remove<T: Any>(&mut self, handle: Handle<T>) -> Option<T> {
        self.storage
            .remove(handle.id)
            .and_then(|asset| asset.as_any_box().downcast().ok())
            .map(|asset| *asset)
    }
}

pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_resource(Assets::new());
        app.add_resource(AssetLoader::new(app.world().clone()));
        Ok(())
    }
}
