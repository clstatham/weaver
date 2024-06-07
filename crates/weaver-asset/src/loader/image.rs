use std::path::Path;

use weaver_core::texture::Texture;
use weaver_util::prelude::Result;

pub fn load_png(path: impl AsRef<Path>) -> Result<Texture> {
    let path = path.as_ref();
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    let image = image::load(&mut reader, image::ImageFormat::Png)?;

    Ok(Texture {
        image: image.into_rgba8(),
    })
}
