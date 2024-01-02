use std::{io::Read, path::Path, sync::Arc};

use image::codecs::hdr::HdrDecoder;
use weaver_proc_macro::Component;

use super::color::Color;

struct TextureInner {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

#[derive(Clone, Component)]
pub struct Texture {
    inner: Arc<TextureInner>,
}

impl Texture {
    pub const WINDOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
    pub const SDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
    pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
    pub const NORMAL_MAP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn load(
        path: impl AsRef<Path>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: Option<&str>,
        is_normal_map: bool,
    ) -> Self {
        let path = path.as_ref();
        let label = label.unwrap_or_else(|| path.to_str().unwrap());

        let image = image::open(path).unwrap().flipv().to_rgba8();
        let (width, height) = image.dimensions();

        Self::from_data_rgba8(
            width as usize,
            height as usize,
            &image,
            device,
            queue,
            Some(label),
            is_normal_map,
        )
    }

    pub fn from_data_rgba8(
        width: usize,
        height: usize,
        data: &[u8],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: Option<&str>,
        is_normal_map: bool,
    ) -> Self {
        let texture_extent = wgpu::Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        };

        let format = if is_normal_map {
            Self::NORMAL_MAP_FORMAT
        } else {
            Self::SDR_FORMAT
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: texture_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        queue.write_texture(
            texture.as_image_copy(),
            data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width as u32),
                rows_per_image: Some(height as u32),
            },
            texture_extent,
        );

        Self {
            inner: Arc::new(TextureInner { texture, view }),
        }
    }

    pub fn from_data_r8g8b8(
        width: usize,
        height: usize,
        data: &[u8],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: Option<&str>,
        is_normal_map: bool,
    ) -> Self {
        // convert the data to RGBA
        let mut rgba = Vec::with_capacity(width * height * 4);
        for pixel in data.chunks(3) {
            rgba.extend_from_slice(pixel);
            rgba.push(255);
        }

        Self::from_data_rgba8(width, height, &rgba, device, queue, label, is_normal_map)
    }

    pub fn solid_color(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color: Color,
        width: u32,
        height: u32,
        label: Option<&str>,
    ) -> Self {
        let (r, g, b) = color.rgb_int();
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..width * height {
            data.extend_from_slice(&[r, g, b, 255]);
        }

        Self::from_data_rgba8(
            width as usize,
            height as usize,
            &data,
            device,
            queue,
            label,
            false,
        )
    }

    pub fn default_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
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
            width as usize,
            height as usize,
            &data,
            device,
            queue,
            Some("Default Texture"),
            false,
        )
    }

    pub fn create_color_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        label: Option<&str>,
        usage: wgpu::TextureUsages,
        format: Option<wgpu::TextureFormat>,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: format.unwrap_or(Self::SDR_FORMAT),
            usage,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            inner: Arc::new(TextureInner { texture, view }),
        }
    }

    pub fn create_depth_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        label: Option<&str>,
        usage: wgpu::TextureUsages,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            inner: Arc::new(TextureInner { texture, view }),
        }
    }

    pub fn create_normal_texture(
        device: &wgpu::Device,
        width: usize,
        height: usize,
        label: Option<&str>,
        usage: wgpu::TextureUsages,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::NORMAL_MAP_FORMAT,
            usage,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            inner: Arc::new(TextureInner { texture, view }),
        }
    }

    pub fn create_cube_texture(
        device: &wgpu::Device,
        width: usize,
        height: usize,
        label: Option<&str>,
        usage: wgpu::TextureUsages,
        format: Option<wgpu::TextureFormat>,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 6,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: format.unwrap_or(Self::SDR_FORMAT),
            usage,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label,
            dimension: Some(wgpu::TextureViewDimension::Cube),
            array_layer_count: Some(6),
            ..Default::default()
        });

        Self {
            inner: Arc::new(TextureInner { texture, view }),
        }
    }

    pub fn new_hdr_cubemap(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        view_dimension: Option<wgpu::TextureViewDimension>,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 6,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR Cube Map"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Texture::HDR_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("HDR Cube Map View"),
            dimension: view_dimension,
            ..Default::default()
        });

        Self {
            inner: Arc::new(TextureInner { texture, view }),
        }
    }

    pub fn new_depth_cubemap(device: &wgpu::Device, size: u32) -> Self {
        let size = wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 6,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Cube Map"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Texture::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Depth Cube Map View"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        Self {
            inner: Arc::new(TextureInner { texture, view }),
        }
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.inner.texture
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.inner.view
    }
}

pub struct HdrLoader {
    pub(crate) pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
}

impl HdrLoader {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("hdr_loader.wgsl"));

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HDR Loader Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: Texture::HDR_FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HDR Loader Pipeline Layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("HDR Loader Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });

        Self { pipeline, layout }
    }

    pub fn load(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        dst_size: u32,
        path: impl AsRef<Path>,
    ) -> anyhow::Result<Texture> {
        let mut file = std::fs::File::open(path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        drop(file);

        let hdr_decoder = HdrDecoder::new(buf.as_slice())?;
        let meta = hdr_decoder.metadata();
        let mut pixels = vec![[0.0, 0.0, 0.0, 0.0]; meta.width as usize * meta.height as usize];
        hdr_decoder.read_image_transform(
            |pix| {
                let rgb = pix.to_hdr();
                [rgb[0], rgb[1], rgb[2], 1.0f32]
            },
            &mut pixels,
        )?;

        let src = Texture::create_color_texture(
            device,
            meta.width,
            meta.height,
            Some("HDR Source Texture"),
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            Some(Texture::HDR_FORMAT),
        );

        queue.write_texture(
            src.texture().as_image_copy(),
            bytemuck::cast_slice(&pixels),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(meta.width * std::mem::size_of::<[f32; 4]>() as u32),
                rows_per_image: Some(meta.height),
            },
            wgpu::Extent3d {
                width: meta.width,
                height: meta.height,
                depth_or_array_layers: 1,
            },
        );

        let dst = Texture::new_hdr_cubemap(
            device,
            dst_size,
            dst_size,
            Some(wgpu::TextureViewDimension::D2Array),
        );

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HDR Loader Bind Group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(src.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(dst.view()),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("HDR Loader Encoder"),
        });

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("HDR Loader Compute Pass"),
            });
            let num_workgroups = (dst_size + 15) / 16;
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(num_workgroups, num_workgroups, 6);
        }

        queue.submit(std::iter::once(encoder.finish()));

        Ok(dst)
    }
}
