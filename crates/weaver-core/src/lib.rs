use color::Color;
use mesh::Mesh;
use transform::Transform;
use weaver_app::{plugin::Plugin, App};
use weaver_util::prelude::Result;

pub mod color;
pub mod input;
pub mod mesh;
pub mod texture;
pub mod time;
pub mod transform;

pub mod prelude {
    pub use crate::color::*;
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
        Ok(())
    }
}
