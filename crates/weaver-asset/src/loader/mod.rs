use std::{path::Path, rc::Rc};

use weaver_ecs::world::World;
use weaver_util::prelude::*;

use crate::{Asset, Assets, Handle, UntypedHandle};

pub mod mesh;

pub struct AssetLoader {
    world: Rc<World>,
}

impl AssetLoader {
    pub fn new(world: Rc<World>) -> Self {
        Self { world }
    }

    pub fn load<T: Asset>(&self, path: impl AsRef<Path>) -> Result<Handle<T>> {
        let untyped = self.load_untyped(path)?;
        Handle::<T>::try_from(untyped)
    }

    fn load_untyped(&self, path: impl AsRef<Path>) -> Result<UntypedHandle> {
        let path = path.as_ref();
        let mut assets = self.world.get_resource_mut::<Assets>().unwrap();

        if let Some(handle) = assets.find_by_path(path) {
            return Ok(handle);
        }

        match path.extension().and_then(|ext| ext.to_str()) {
            Some("obj") => {
                let asset = mesh::load_obj(path)?;
                let handle = assets.insert(asset, path);
                Ok(handle.into())
            }
            _ => bail!("unsupported file extension: {:?}", path),
        }
    }
}
