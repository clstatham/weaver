//! GPU textures: a texture-plus-view wrapper, texture arrays, and the [`GpuImage`] render asset.
//!
//! [`GpuTexture`] carries a [`GpuResourceId`] so bind groups can tell when a texture was replaced.
//! [`GpuImage`] uploads a [`Texture`] asset once and exposes it through both an sRGB and a linear
//! view of the same data, so a material picks the interpretation (color vs. data) at bind time.

use std::sync::Arc;

use weaver_app::{plugin::Plugin, prelude::App};
use weaver_core::texture::Texture;
use weaver_util::prelude::*;
use wgpu::util::DeviceExt;

use crate::{
    GpuResourceId,
    asset::{PrepareAssetError, RenderAsset, RenderAssetPlugin},
};

/// The texture formats the renderer uses, by role.
pub mod texture_format {
    pub use wgpu::TextureFormat;
    /// The swapchain format. sRGB, so the hardware encodes on write.
    pub const VIEW_FORMAT: TextureFormat = TextureFormat::Bgra8UnormSrgb;
    /// 8-bit color textures, sampled as sRGB.
    pub const SDR_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;
    /// Format of the per-view HDR color target.
    pub const HDR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
    /// High-precision format for HDR cubemaps.
    pub const HDR_CUBE_FORMAT: TextureFormat = TextureFormat::Rgba32Float;
    /// Normal maps. Same as [`LINEAR_FORMAT`].
    pub const NORMAL_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;
    /// RGBA8 data that isn't color (normals, masks).
    pub const LINEAR_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;
    /// Format of the per-view depth target.
    pub const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;
}

/// A texture and a view of it. Clones share both and the [`GpuResourceId`].
#[derive(Clone)]
pub struct GpuTexture {
    /// The underlying texture.
    pub texture: Arc<wgpu::Texture>,
    /// The view bound when this texture is used.
    pub view: Arc<wgpu::TextureView>,
    id: GpuResourceId,
}

impl GpuTexture {
    /// Wraps an existing texture and view, giving them a fresh [`GpuResourceId`].
    pub fn from_parts(texture: wgpu::Texture, view: wgpu::TextureView) -> Self {
        Self {
            texture: Arc::new(texture),
            view: Arc::new(view),
            id: GpuResourceId::new(),
        }
    }

    /// Returns the id that bind groups compare to detect a replaced texture.
    pub fn id(&self) -> GpuResourceId {
        self.id
    }

    /// The same texture (and id) seen through a different view.
    pub fn with_view(&self, view: wgpu::TextureView) -> Self {
        Self {
            texture: self.texture.clone(),
            view: Arc::new(view),
            id: self.id,
        }
    }

    /// Creates an empty 2D texture with one mip level and one sample, plus its default view.
    pub fn new(
        device: &wgpu::Device,
        label: Option<&str>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self::from_parts(texture, view)
    }

    /// Uploads `image` (converted to RGBA8) as a 2D texture in `format`, usable for binding.
    ///
    /// Always returns `Some`.
    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &Texture,
        format: wgpu::TextureFormat,
    ) -> Option<Self> {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Texture"),
                size: wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &image.to_rgba8(),
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Some(Self::from_parts(texture, view))
    }

    /// Returns the texture's format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.texture.format()
    }
}

/// A 2D texture array with one layer per source image, viewed as a `texture_2d_array`.
///
/// Unlike [`GpuTexture`] it has no [`GpuResourceId`], so bind groups can't tell when it's replaced.
pub struct GpuTextureArray {
    /// The array texture.
    pub backing_texture: Arc<wgpu::Texture>,
    /// A `D2Array` view over all layers.
    pub backing_view: Arc<wgpu::TextureView>,
}

impl GpuTextureArray {
    /// Uploads `images` as the layers of an `Rgba8UnormSrgb` array, in order.
    ///
    /// Always returns `Some`.
    ///
    /// # Panics
    ///
    /// Panics if `images` is empty or the images don't all have the same width and height.
    pub fn from_images(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        images: &[Texture],
    ) -> Option<Self> {
        let layer_count = images.len() as u32;
        assert!(
            layer_count > 0,
            "Texture array must have at least one layer"
        );
        assert!(
            images
                .iter()
                .all(|image| image.width() == images[0].width()),
            "All images in texture array must have the same width"
        );
        assert!(
            images
                .iter()
                .all(|image| image.height() == images[0].height()),
            "All images in texture array must have the same height"
        );
        let width = images[0].width();
        let height = images[0].height();

        let data = images
            .iter()
            .flat_map(|image| image.to_rgba8())
            .collect::<Vec<u8>>();

        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Texture Array"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: layer_count,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &data,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(layer_count),
            base_array_layer: 0,
            ..Default::default()
        });

        Some(Self {
            backing_texture: Arc::new(texture),
            backing_view: Arc::new(view),
        })
    }

    /// Returns the texture format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.backing_texture.format()
    }

    /// Returns the number of layers.
    pub fn layer_count(&self) -> u32 {
        self.backing_texture.size().depth_or_array_layers
    }

    /// Returns the width of each layer in pixels.
    pub fn width(&self) -> u32 {
        self.backing_texture.size().width
    }

    /// Returns the height of each layer in pixels.
    pub fn height(&self) -> u32 {
        self.backing_texture.size().height
    }

    /// Returns the array view.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.backing_view
    }

    /// Returns the array view as a bind group resource.
    pub fn as_binding_resource(&'_ self) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::TextureView(&self.backing_view)
    }
}

/// The GPU version of a [`Texture`] asset: RGBA8, viewable as sRGB (for colors) or linear (for
/// data such as normal maps). Uploaded once, however many materials use it.
pub struct GpuImage {
    /// The image read as `Rgba8UnormSrgb` (base color, emissive).
    pub srgb: GpuTexture,
    /// The image read as `Rgba8Unorm` (normals, roughness, masks). Same texture and id as `srgb`.
    pub linear: GpuTexture,
}

impl RenderAsset for GpuImage {
    type Source = Texture;
    type Param = ();

    fn prepare(
        image: Texture,
        _: &mut (),
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Self, PrepareAssetError<Texture>> {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Image"),
                size: wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: texture_format::LINEAR_FORMAT,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[texture_format::SDR_FORMAT],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &image.to_rgba8(),
        );

        let linear_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let srgb_view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(texture_format::SDR_FORMAT),
            ..Default::default()
        });
        let linear = GpuTexture::from_parts(texture, linear_view);
        let srgb = linear.with_view(srgb_view);

        Ok(Self { srgb, linear })
    }
}

/// Registers [`GpuImage`] as a render asset.
pub struct TexturePlugin;

impl Plugin for TexturePlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(RenderAssetPlugin::<GpuImage>::default())?;
        Ok(())
    }
}
