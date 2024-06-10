use std::path::Path;

use weaver_asset::{
    loader::LoadAsset,
    prelude::{Asset, Assets},
    UntypedHandle,
};
use weaver_util::prelude::Result;

#[derive(Asset)]
pub struct Texture {
    pub image: image::RgbaImage,
}

impl Texture {
    pub fn new(image: image::RgbaImage) -> Self {
        Self { image }
    }

    pub fn from_rgba8(rgba8: &[u8], width: u32, height: u32) -> Self {
        let image = image::RgbaImage::from_raw(width, height, rgba8.to_vec()).unwrap();
        Self { image }
    }

    pub fn to_rgba8(&self) -> Vec<u8> {
        self.image.clone().into_raw()
    }

    pub fn from_rgb8(rgb8: &[u8], width: u32, height: u32) -> Self {
        let mut rgba8 = Vec::new();

        for i in 0..(width * height) as usize {
            rgba8.push(rgb8[i * 3]);
            rgba8.push(rgb8[i * 3 + 1]);
            rgba8.push(rgb8[i * 3 + 2]);
            rgba8.push(255);
        }

        Self::from_rgba8(&rgba8, width, height)
    }

    pub fn to_rgb8(&self) -> Vec<u8> {
        let mut rgb8 = Vec::new();

        for i in 0..self.image.width() * self.image.height() {
            let pixel = self
                .image
                .get_pixel(i % self.image.width(), i / self.image.width());
            rgb8.push(pixel[0]);
            rgb8.push(pixel[1]);
            rgb8.push(pixel[2]);
        }

        rgb8
    }

    pub fn width(&self) -> u32 {
        self.image.width()
    }

    pub fn height(&self) -> u32 {
        self.image.height()
    }

    pub fn resize(&mut self, width: u32, height: u32, filter: image::imageops::FilterType) {
        self.image = image::imageops::resize(&self.image, width, height, filter);
    }
}

pub struct TextureLoader;

impl LoadAsset for TextureLoader {
    fn load_asset(&self, path: &Path, assets: &mut Assets) -> Result<UntypedHandle> {
        let image = image::open(path)?;
        let image = image.to_rgba8();
        let texture = Texture::from_rgba8(&image, image.width(), image.height());
        Ok(assets.insert(texture, Some(path)).into())
    }
}