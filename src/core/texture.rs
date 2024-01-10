use std::path::Path;

use crate::{
    ecs::Component,
    renderer::{CreateBindGroupLayout, LazyBufferHandle},
};

use super::color::Color;

pub trait TextureFormat: 'static + Send + Sync + Component {
    const FORMAT: wgpu::TextureFormat;
}

macro_rules! texture_formats {
    ($($name:ident: $format:ident,)*) => {
        $(
            #[derive(Debug, Clone, Copy, Component)]
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
    HdrFormat: Rgba16Float,
    PositionMapFormat: Rgba32Float,
    NormalMapFormat: Rgba8Unorm,
    DepthFormat: Depth32Float,
    HdrD2ArrayFormat: Rgba32Float,
    HdrCubeFormat: Rgba32Float,
    MonoFormat: R16Float,
    MonoCubeFormat: R16Float,
    DepthCubeFormat: Depth32Float,
    MonoCubeArrayFormat: R16Float,
    DepthCubeArrayFormat: Depth32Float,
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

impl CreateBindGroupLayout for HdrD2ArrayFormat {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HDR Cubemap Texture Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            }],
        })
    }
}

impl CreateBindGroupLayout for HdrCubeFormat {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HDR Cubemap Texture Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    multisampled: false,
                },
                count: None,
            }],
        })
    }
}

impl CreateBindGroupLayout for MonoFormat {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Mono Texture Bind Group Layout"),
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

impl CreateBindGroupLayout for MonoCubeFormat {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Mono Cubemap Texture Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    multisampled: false,
                },
                count: None,
            }],
        })
    }
}

impl CreateBindGroupLayout for DepthCubeFormat {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Depth Cubemap Texture Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    multisampled: false,
                },
                count: None,
            }],
        })
    }
}

impl CreateBindGroupLayout for PositionMapFormat {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Position Map Texture Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
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

impl CreateBindGroupLayout for MonoCubeArrayFormat {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Mono Cubemap Array Texture Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::CubeArray,
                    multisampled: false,
                },
                count: None,
            }],
        })
    }
}

impl CreateBindGroupLayout for DepthCubeArrayFormat {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Depth Cubemap Array Texture Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::CubeArray,
                    multisampled: false,
                },
                count: None,
            }],
        })
    }
}

#[derive(Clone, Component)]
pub struct Texture {
    pub(crate) handle: LazyBufferHandle,
}

impl Texture {
    pub fn from_handle(handle: LazyBufferHandle) -> Self {
        Self { handle }
    }

    pub fn load(
        path: impl AsRef<Path>,
        format: wgpu::TextureFormat,
        label: Option<&'static str>,
    ) -> Self {
        let path = path.as_ref();

        let image = image::open(path).unwrap().flipv().to_rgba8();
        let (width, height) = image.dimensions();

        Self::from_data_rgba8(width, height, &image, format, label)
    }

    pub fn from_data_rgba8(
        width: u32,
        height: u32,
        data: &[u8],
        format: wgpu::TextureFormat,
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
                format,
                dimension: wgpu::TextureDimension::D2,
                view_dimension: wgpu::TextureViewDimension::D2,
                depth_or_array_layers: 1,
            },
            label,
            Some(data.into()),
        );
        Self { handle }
    }

    pub fn from_data_r8g8b8(
        width: u32,
        height: u32,
        data: &[u8],
        format: wgpu::TextureFormat,
        label: Option<&'static str>,
    ) -> Self {
        // convert the data to RGBA
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for pixel in data.chunks(3) {
            rgba.extend_from_slice(pixel);
            rgba.push(255);
        }

        Self::from_data_rgba8(width, height, &rgba, format, label)
    }

    pub fn solid_color(
        color: Color,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        label: Option<&'static str>,
    ) -> Self {
        let (r, g, b) = color.rgb_int();
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..width * height {
            data.extend_from_slice(&[r, g, b, 255]);
        }

        Self::from_data_rgba8(width, height, &data, format, label)
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

        Self::from_data_rgba8(
            width,
            height,
            &data,
            SdrFormat::FORMAT,
            Some("Default Texture"),
        )
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
        Self { handle }
    }
}

#[derive(Clone, Component)]
pub struct Skybox {
    pub texture: Texture,
}
