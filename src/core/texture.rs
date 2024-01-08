use std::path::Path;

use weaver_proc_macro::Component;

use crate::renderer::{
    AllocBuffers, BufferHandle, CreateBindGroupLayout, LazyBufferHandle, Renderer,
};

use super::color::Color;

pub trait TextureFormat: 'static + Send + Sync {
    const FORMAT: wgpu::TextureFormat;
}

macro_rules! texture_formats {
    ($($name:ident: $format:ident,)*) => {
        $(
            #[derive(Debug, Clone, Copy)]
            pub struct $name;
            impl TextureFormat for $name {
                const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::$format;
            }
        )*
    }
}

texture_formats! {
    WindowFormat: Bgra8UnormSrgb,
    SdrFormat: Rgba8UnormSrgb,
    HdrFormat: Rgba32Float,
    NormalMapFormat: Rgba8Unorm,
    DepthFormat: Depth32Float,
}

impl CreateBindGroupLayout for WindowFormat {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Window Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        })
    }
}

impl CreateBindGroupLayout for SdrFormat {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SDR Texture Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        })
    }
}

impl CreateBindGroupLayout for HdrFormat {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HDR Texture Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        })
    }
}

impl CreateBindGroupLayout for NormalMapFormat {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Normal Map Texture Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        })
    }
}

impl CreateBindGroupLayout for DepthFormat {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Depth Texture Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        })
    }
}

#[derive(Clone, Component)]
pub struct Texture<F: TextureFormat> {
    pub(crate) handle: LazyBufferHandle,
    _format: std::marker::PhantomData<F>,
}

impl<F: TextureFormat> Texture<F> {
    pub fn load(path: impl AsRef<Path>, label: Option<&'static str>) -> Self {
        let path = path.as_ref();

        let image = image::open(path).unwrap().flipv().to_rgba8();
        let (width, height) = image.dimensions();

        Self::from_data_rgba8(width, height, &image, label)
    }

    pub fn from_data_rgba8(
        width: u32,
        height: u32,
        data: &[u8],
        label: Option<&'static str>,
    ) -> Self {
        let handle = LazyBufferHandle::new(
            crate::renderer::BufferBindingType::Texture {
                width,
                height,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: F::FORMAT,
                dimension: wgpu::TextureDimension::D2,
                view_dimension: wgpu::TextureViewDimension::D2,
                depth_or_array_layers: 1,
            },
            label,
            Some(data.into()),
        );
        Self {
            handle,
            _format: std::marker::PhantomData,
        }
    }

    pub fn from_data_r8g8b8(
        width: u32,
        height: u32,
        data: &[u8],
        label: Option<&'static str>,
    ) -> Self {
        // convert the data to RGBA
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for pixel in data.chunks(3) {
            rgba.extend_from_slice(pixel);
            rgba.push(255);
        }

        Self::from_data_rgba8(width, height, &rgba, label)
    }

    pub fn solid_color(color: Color, width: u32, height: u32, label: Option<&'static str>) -> Self {
        let (r, g, b) = color.rgb_int();
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..width * height {
            data.extend_from_slice(&[r, g, b, 255]);
        }

        Self::from_data_rgba8(width, height, &data, label)
    }

    pub fn default_texture() -> Self {
        let width = 128;
        let height = 128;
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        for x in 0..width {
            for y in 0..height {
                // pink/white checkerboard
                let (r, g, b) = match (x < width / 2, y < height / 2) {
                    (true, true) | (false, false) => (255, 0, 255),
                    (true, false) | (false, true) => (255, 255, 255),
                };
                data.extend_from_slice(&[r, g, b, 255]);
            }
        }

        Self::from_data_rgba8(width, height, &data, Some("Default Texture"))
    }

    pub fn new_lazy(
        width: u32,
        height: u32,
        label: Option<&'static str>,
        usage: wgpu::TextureUsages,
        format: wgpu::TextureFormat,
        dimension: wgpu::TextureDimension,
        view_dimension: wgpu::TextureViewDimension,
        depth_or_array_layers: u32,
    ) -> Self {
        let handle = LazyBufferHandle::new(
            crate::renderer::BufferBindingType::Texture {
                width,
                height,
                usage,
                format,
                dimension,
                view_dimension,
                depth_or_array_layers,
            },
            label,
            None,
        );
        Self {
            handle,
            _format: std::marker::PhantomData,
        }
    }
}

impl<F: TextureFormat + CreateBindGroupLayout> AllocBuffers for Texture<F> {
    fn alloc_buffers(&self, renderer: &Renderer) -> anyhow::Result<Vec<BufferHandle>> {
        Ok(vec![self.handle.get_or_create::<Self>(renderer)])
    }
}

impl<F: TextureFormat + CreateBindGroupLayout> CreateBindGroupLayout for Texture<F> {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        F::create_bind_group_layout(device)
    }
}
