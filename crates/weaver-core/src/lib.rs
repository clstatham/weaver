//! Core engine types shared by the rest of the engine.
//!
//! Everything here is plain data plus the small amount of glue needed to get it into an [`App`]:
//! [`transform::Transform`], [`color::Color`], the bounds and intersection math in [`geometry`],
//! CPU-side [`mesh::Mesh`]es with OBJ and glTF loaders, [`texture::Texture`]s, [`time::Time`] and
//! [`time::FixedTimestep`], and the [`input::Input`] state. The renderer converts these into its
//! own GPU-side forms; nothing in this crate touches the GPU.
//!
//! [`CoreTypesPlugin`] registers the [`Mesh`] and [`Texture`] asset types and their loaders. The
//! time and input resources have their own plugins ([`time::TimePlugin`], [`input::InputPlugin`]).
//!
//! [`prelude`] re-exports every module here plus all of [`glam`], so `Vec3`, `Quat` and `Mat4`
//! come along with it.
//!
//! [`App`]: weaver_app::App

use std::path::PathBuf;

use mesh::Mesh;
use texture::{Texture, TextureLoader};
use weaver_app::{App, plugin::Plugin};
use weaver_asset::{AssetApp, PathAndFilesystem};
use weaver_util::prelude::*;

pub mod color;
pub mod geometry;
pub mod input;
pub mod mesh;
pub mod texture;
pub mod time;
pub mod transform;

/// Everything in this crate, plus all of [`glam`].
pub mod prelude {
    pub use crate::CoreTypesPlugin;
    pub use crate::color::*;
    pub use crate::geometry::*;
    pub use crate::input::*;
    pub use crate::mesh::*;
    pub use crate::texture::*;
    pub use crate::time::*;
    pub use crate::transform::*;

    pub use glam::*;
}

/// Registers the [`Mesh`] and [`Texture`] asset types and the loaders for them.
///
/// Loaders are added for PNG/TGA/etc. textures, OBJ meshes and glTF meshes, each from a file path
/// (through [`PathAndFilesystem`]) and from raw bytes (`Vec<u8>`).
pub struct CoreTypesPlugin;

impl Plugin for CoreTypesPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_asset::<Texture>();
        app.add_asset::<Mesh>();

        app.add_asset_loader::<TextureLoader<PathBuf>, _>();
        app.add_asset_loader::<TextureLoader<Vec<u8>>, _>();
        app.add_asset_loader::<mesh::ObjMeshLoader<PathAndFilesystem>, _>();
        app.add_asset_loader::<mesh::ObjMeshLoader<Vec<u8>>, _>();
        app.add_asset_loader::<mesh::GltfMeshLoader<PathBuf>, _>();
        app.add_asset_loader::<mesh::GltfMeshLoader<Vec<u8>>, _>();
        Ok(())
    }
}
