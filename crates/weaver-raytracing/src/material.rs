use encase::ShaderType;
use weaver_asset::{prelude::Asset, Handle};
use weaver_core::{color::Color, texture::Texture};

pub const WHITE_TEXTURE: Handle<Texture> = Handle::from_u128(0x5a0322640b134bfea5da084ee142d341);
pub const BLACK_TEXTURE: Handle<Texture> = Handle::from_u128(0xb1481b681b554dafb84a7942edfdba2b);
pub const ERROR_TEXTURE: Handle<Texture> = Handle::from_u128(0x0c813992128b48c6bdfeeab4a6233db8);

#[derive(Debug, Clone, Copy, PartialEq, Asset)]
pub struct Material {
    pub diffuse: Color,
}

impl From<Color> for Material {
    fn from(diffuse: Color) -> Self {
        Self { diffuse }
    }
}

impl Default for Material {
    fn default() -> Self {
        Self {
            diffuse: Color::WHITE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, ShaderType)]
#[repr(C)]
pub struct MaterialUniform {
    pub diffuse: Color,
}
