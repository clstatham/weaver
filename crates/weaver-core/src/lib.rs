use color::Color;
use mesh::Mesh;
use transform::Transform;
use weaver_app::{plugin::Plugin, App};
use weaver_asset::AssetApp;
use weaver_util::Result;

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

    pub use glam::*;
}

pub struct CoreTypesPlugin;

impl Plugin for CoreTypesPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.register_type::<Transform>();
        app.register_type::<Color>();
        app.register_type::<Mesh>();
        app.register_type::<geometry::Plane>();
        app.register_type::<geometry::Ray>();
        app.register_type::<geometry::Aabb>();

        app.add_asset_loader::<texture::Texture, texture::TextureLoader>();
        app.add_asset_loader::<Mesh, mesh::ObjMeshLoader>();
        app.add_asset_loader::<Mesh, mesh::GltfMeshLoader>();
        Ok(())
    }
}
