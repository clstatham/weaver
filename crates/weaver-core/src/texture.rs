use std::path::PathBuf;

use weaver_asset::{
    prelude::{Asset, Loader},
    AssetLoadQueues, Filesystem, LoadSource, PathAndFilesystem,
};
use weaver_ecs::prelude::Resource;
use weaver_util::Result;

#[derive(Debug, Clone, Asset)]
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

#[derive(Resource, Default)]
pub struct TextureLoader<S: LoadSource>(std::marker::PhantomData<S>);

impl Loader<Texture, PathAndFilesystem> for TextureLoader<PathBuf> {
    fn load(
        &self,
        source: PathAndFilesystem,
        _load_queues: &AssetLoadQueues<'_>,
    ) -> Result<Texture> {
        let bytes = source.read()?;
        load_texture_common(&bytes)
    }
}

impl Loader<Texture, Vec<u8>> for TextureLoader<Vec<u8>> {
    fn load(&self, source: Vec<u8>, _load_queues: &AssetLoadQueues<'_>) -> Result<Texture> {
        load_texture_common(&source)
    }
}

fn load_texture_common(bytes: &[u8]) -> Result<Texture> {
    // check if it's a tga file
    if let Ok(image) = image::load_from_memory_with_format(bytes, image::ImageFormat::Tga) {
        log::trace!(
            "Successfully loaded TGA texture with dimensions {}x{}",
            image.width(),
            image.height()
        );
        return Ok(Texture::from_rgba8(
            &image.to_rgba8(),
            image.width(),
            image.height(),
        ));
    }
    let image = image::load_from_memory(bytes)?;
    let image = image.to_rgba8();
    log::trace!(
        "Successfully loaded texture with dimensions {}x{}",
        image.width(),
        image.height()
    );
    Ok(Texture::from_rgba8(&image, image.width(), image.height()))
}
