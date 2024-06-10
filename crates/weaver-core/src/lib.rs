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
