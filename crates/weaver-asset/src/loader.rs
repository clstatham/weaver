use std::{path::Path, sync::Arc};

use weaver_ecs::{prelude::Resource, world::World};
use weaver_util::prelude::*;

use crate::{Asset, Assets, Handle, UntypedHandle};

#[derive(Resource)]
pub struct AssetLoader {
    world: Arc<World>,
    loaders: Vec<Arc<dyn LoadAsset>>,
}

impl AssetLoader {
    pub fn new(world: Arc<World>) -> Self {
        Self {
            world,
            loaders: Vec::new(),
        }
    }

    pub fn add_loader<L>(&mut self, loader: L)
    where
        L: LoadAsset + 'static,
    {
        self.loaders.push(Arc::new(loader));
    }

    pub fn load<T: Asset>(&self, path: impl AsRef<Path>) -> Result<Handle<T>> {
        let untyped = self.load_untyped(path)?;
        Handle::<T>::try_from(untyped)
    }

    fn load_untyped(&self, path: impl AsRef<Path>) -> Result<UntypedHandle> {
        let path = path.as_ref();
        let mut assets = self.world.get_resource_mut::<Assets>().unwrap();

        for loader in &self.loaders {
            if let Ok(handle) = loader.load_asset(path, &mut assets) {
                return Ok(handle);
            }
        }

        bail!("no loader found for asset: {:?}", path);
    }
}

pub trait LoadAsset: Send + Sync {
    fn load_asset(&self, path: &Path, assets: &mut Assets) -> Result<UntypedHandle>;
}
