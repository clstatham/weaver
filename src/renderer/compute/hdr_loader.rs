use std::{io::Read, path::Path};

use image::codecs::hdr::HdrDecoder;
use weaver_proc_macro::Resource;

use crate::{
    core::texture::{HdrCubeFormat, HdrD2ArrayFormat, Texture, TextureFormat},
    renderer::{BufferStorage, LazyBufferHandle, Renderer},
};

#[derive(Resource)]
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
                        format: HdrCubeFormat::FORMAT,
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
        renderer: &Renderer,
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

        let src = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR Loader Source Texture"),
            size: wgpu::Extent3d {
                width: meta.width,
                height: meta.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HdrCubeFormat::FORMAT,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let src_view = src.create_view(&wgpu::TextureViewDescriptor {
            label: Some("HDR Loader Source Texture View"),
            format: Some(HdrCubeFormat::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        renderer.queue.write_texture(
            src.as_image_copy(),
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

        let dst_buf = renderer.create_texture::<HdrCubeFormat>(
            dst_size,
            dst_size,
            HdrD2ArrayFormat::FORMAT,
            wgpu::TextureDimension::D2,
            wgpu::TextureViewDimension::Cube,
            6,
            wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::STORAGE_BINDING,
            Some("HDR Loader Destination Texture"),
        );

        let dst_view = match &*dst_buf.storage {
            BufferStorage::Texture { texture, .. } => {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("HDR Loader Destination Texture View"),
                    format: Some(HdrD2ArrayFormat::FORMAT),
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    aspect: wgpu::TextureAspect::All,
                    array_layer_count: Some(6),
                    ..Default::default()
                })
            }
            _ => unreachable!(),
        };

        let bind_group = renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("HDR Loader Bind Group"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&src_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&dst_view),
                    },
                ],
            });

        let mut encoder = renderer
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("HDR Loader Encoder"),
            });

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("HDR Loader Compute Pass"),
                timestamp_writes: None,
            });
            let num_workgroups = (dst_size + 15) / 16;
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(num_workgroups, num_workgroups, 6);
        }

        renderer.queue.submit(std::iter::once(encoder.finish()));

        let handle = renderer.buffer_allocator.insert_buffer(dst_buf);

        let handle = LazyBufferHandle::from_handle(
            handle,
            crate::renderer::BufferBindingType::Texture {
                width: dst_size,
                height: dst_size,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: HdrD2ArrayFormat::FORMAT,
                dimension: wgpu::TextureDimension::D2,
                view_dimension: wgpu::TextureViewDimension::D2Array,
                depth_or_array_layers: 1,
            },
            Some("HDR Loader Destination Texture"),
        );

        Ok(Texture::from_handle(handle))
    }
}
