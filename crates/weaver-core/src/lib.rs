use std::path::PathBuf;

use mesh::Mesh;
use texture::{Texture, TextureLoader};
use weaver_app::{plugin::Plugin, App};
use weaver_asset::AssetApp;
use weaver_util::prelude::*;

pub mod color;
pub mod geometry;
pub mod input;
pub mod mesh;
pub mod texture;
pub mod time;
pub mod transform;

pub mod prelude {
    pub use crate::color::*;
    pub use crate::geometry::*;
    pub use crate::input::*;
    pub use crate::mesh::*;
    pub use crate::texture::*;
    pub use crate::time::*;
    pub use crate::transform::*;
    pub use crate::CoreTypesPlugin;

    pub use glam::*;
}

pub struct CoreTypesPlugin;

impl Plugin for CoreTypesPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_asset::<Texture>();
        app.add_asset::<Mesh>();

        app.add_asset_loader::<Texture, TextureLoader<PathBuf>, _>();
        app.add_asset_loader::<Texture, TextureLoader<Vec<u8>>, _>();
        app.add_asset_loader::<Mesh, mesh::ObjMeshLoader<PathBuf>, _>();
        app.add_asset_loader::<Mesh, mesh::ObjMeshLoader<Vec<u8>>, _>();
        app.add_asset_loader::<Mesh, mesh::GltfMeshLoader<PathBuf>, _>();
        app.add_asset_loader::<Mesh, mesh::GltfMeshLoader<Vec<u8>>, _>();
        Ok(())
    }
}
