use std::{path::Path, rc::Rc};

use weaver_ecs::{component::Component, world::World};
use weaver_util::prelude::*;

use crate::{Asset, Assets, Handle, UntypedHandle};

pub mod image;
pub mod mesh;

pub struct AssetLoader {
    world: Rc<World>,
    loaders: Vec<Box<dyn LoadAsset>>,
}

impl AssetLoader {
    pub fn new(world: Rc<World>) -> Self {
        Self {
            world,
            loaders: Vec::new(),
        }
    }

    pub fn add_loader<L>(&mut self, loader: L)
    where
        L: LoadAsset + 'static,
    {
        self.loaders.push(Box::new(loader));
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
                let asset = self::mesh::load_obj(path)?;
                let handle = assets.insert(asset, Some(path));
                Ok(handle.into())
            }
            Some("png") => {
                let asset = self::image::load_png(path)?;
                let handle = assets.insert(asset, Some(path));
                Ok(handle.into())
            }
            _ => {
                for loader in &self.loaders {
                    if let Ok(handle) = loader.load_asset(path, &mut assets) {
                        return Ok(handle);
                    }
                }

                Err(anyhow!("No loader found for {:?}", path))
            }
        }
    }
}

pub trait LoadAsset: Component {
    fn load_asset(&self, path: &Path, assets: &mut Assets) -> Result<UntypedHandle>;
}
