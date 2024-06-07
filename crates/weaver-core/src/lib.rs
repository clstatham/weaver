pub mod color;
pub mod mesh;
pub mod texture;

pub mod prelude {
    pub use crate::color::*;
    pub use crate::texture::*;
    pub use glam::*;
}
