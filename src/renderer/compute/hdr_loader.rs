use std::{io::Read, num::NonZeroU32, path::Path};

use image::codecs::hdr::HdrDecoder;
use weaver_proc_macro::Resource;
use wgpu::util::DeviceExt;

use crate::{
    core::texture::{HdrCubeTexture, HdrD2ArrayTexture, Texture, TextureFormat},
    renderer::internals::{GpuComponent, GpuResource, GpuResourceManager, LazyGpuHandle},
};

#[derive(Resource)]
pub struct HdrLoader {
    pub(crate) load_pipeline: wgpu::ComputePipeline,
    load_layout: wgpu::BindGroupLayout,

    pub(crate) irradiance_pipeline: wgpu::RenderPipeline,
    irradiance_layout: wgpu::BindGroupLayout,
    irradiance_views: wgpu::Buffer,
    irradiance_projection: wgpu::Buffer,
    irradiance_transforms_bind_group: wgpu::BindGroup,
}

impl HdrLoader {
    pub fn new(device: &wgpu::Device) -> Self {
        let load_shader = device.create_shader_module(wgpu::include_wgsl!("hdr_loader.wgsl"));

        let load_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                        format: HdrCubeTexture::FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
            ],
        });

        let load_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HDR Loader Pipeline Layout"),
            bind_group_layouts: &[&load_layout],
            push_constant_ranges: &[],
        });

        let load_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("HDR Loader Pipeline"),
            layout: Some(&load_pipeline_layout),
            module: &load_shader,
            entry_point: "load",
        });

        let irradiance_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HDR Loader Bind Group Layout"),
            entries: &[
                // src
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                // sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        let irradiance_transforms_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("HDR Loader Irradiance Transforms Bind Group Layout"),
                entries: &[
                    // views
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // projection
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // let view_transform = match i {
        //     // right
        //     0 => point_light.view_transform_in_direction(glam::Vec3::X, glam::Vec3::Y),
        //     // left
        //     1 => point_light.view_transform_in_direction(-glam::Vec3::X, glam::Vec3::Y),
        //     // top
        //     2 => point_light.view_transform_in_direction(glam::Vec3::Y, -glam::Vec3::Z),
        //     // bottom
        //     3 => point_light.view_transform_in_direction(-glam::Vec3::Y, glam::Vec3::Z),
        //     // front
        //     4 => point_light.view_transform_in_direction(glam::Vec3::Z, glam::Vec3::Y),
        //     // back
        //     5 => point_light.view_transform_in_direction(-glam::Vec3::Z, glam::Vec3::Y),
        //     _ => unreachable!(),
        // };
        let views = (0..6)
            .map(|i| match i {
                // right
                0 => glam::Mat4::look_at_rh(glam::Vec3::ZERO, glam::Vec3::X, glam::Vec3::Y),
                // left
                1 => glam::Mat4::look_at_rh(glam::Vec3::ZERO, -glam::Vec3::X, glam::Vec3::Y),
                // top
                2 => glam::Mat4::look_at_rh(glam::Vec3::ZERO, glam::Vec3::Y, -glam::Vec3::Z),
                // bottom
                3 => glam::Mat4::look_at_rh(glam::Vec3::ZERO, -glam::Vec3::Y, glam::Vec3::Z),
                // front
                4 => glam::Mat4::look_at_rh(glam::Vec3::ZERO, glam::Vec3::Z, glam::Vec3::Y),
                // back
                5 => glam::Mat4::look_at_rh(glam::Vec3::ZERO, -glam::Vec3::Z, glam::Vec3::Y),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();

        let views = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("HDR Loader Irradiance Views"),
            contents: bytemuck::cast_slice(&views),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let irradiance_projection = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("HDR Loader Irradiance Projection"),
            contents: bytemuck::cast_slice(&[glam::Mat4::perspective_rh(
                std::f32::consts::FRAC_PI_2,
                1.0,
                0.1,
                10.0,
            )]),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let irradiance_transforms_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("HDR Loader Irradiance Transforms Bind Group"),
                layout: &irradiance_transforms_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(views.as_entire_buffer_binding()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(
                            irradiance_projection.as_entire_buffer_binding(),
                        ),
                    },
                ],
            });

        let irradiance_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HDR Loader Shader Module"),
            source: wgpu::ShaderSource::Wgsl(include_str!("hdr_irradiance.wgsl").into()),
        });

        let irradiance_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("HDR Loader Pipeline Layout"),
                bind_group_layouts: &[&irradiance_layout, &irradiance_transforms_layout],
                push_constant_ranges: &[],
            });

        let irradiance_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HDR Loader Pipeline"),
            layout: Some(&irradiance_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &irradiance_shader,
                entry_point: "irradiance_vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &irradiance_shader,
                entry_point: "irradiance_fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: HdrCubeTexture::FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: Some(NonZeroU32::new(6).unwrap()),
        });

        Self {
            load_pipeline,
            load_layout,
            irradiance_pipeline,
            irradiance_layout,
            irradiance_views: views,
            irradiance_projection,
            irradiance_transforms_bind_group,
        }
    }

    pub fn load(
        &self,
        resource_manager: &GpuResourceManager,
        dst_size: u32,
        path: impl AsRef<Path>,
    ) -> anyhow::Result<HdrCubeTexture> {
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

        let src = resource_manager
            .device()
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("HDR Loader Source Texture"),
                size: wgpu::Extent3d {
                    width: meta.width,
                    height: meta.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HdrCubeTexture::FORMAT,
                usage: wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

        let src_view = src.create_view(&wgpu::TextureViewDescriptor {
            label: Some("HDR Loader Source Texture View"),
            format: Some(HdrCubeTexture::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        resource_manager.queue().write_texture(
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

        let dst_buf = resource_manager.create_texture(
            dst_size,
            dst_size,
            HdrD2ArrayTexture::FORMAT,
            wgpu::TextureDimension::D2,
            6,
            wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::STORAGE_BINDING,
            Some("HDR Loader Destination Texture"),
        );

        let dst_view = match &*dst_buf {
            GpuResource::Texture { texture, .. } => {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("HDR Loader Destination Texture View"),
                    format: Some(HdrD2ArrayTexture::FORMAT),
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    aspect: wgpu::TextureAspect::All,
                    array_layer_count: Some(6),
                    ..Default::default()
                })
            }
            _ => unreachable!(),
        };

        let bind_group = resource_manager
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("HDR Loader Bind Group"),
                layout: &self.load_layout,
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

        let mut encoder =
            resource_manager
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("HDR Loader Encoder"),
                });

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("HDR Loader Compute Pass"),
                timestamp_writes: None,
            });
            let num_workgroups = (dst_size + 15) / 16;
            cpass.set_pipeline(&self.load_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(num_workgroups, num_workgroups, 6);
        }

        resource_manager
            .queue()
            .submit(std::iter::once(encoder.finish()));

        let handle = resource_manager.insert_resource(dst_buf);

        let handle = LazyGpuHandle::new_ready(handle);

        Ok(HdrCubeTexture::from_texture(Texture::from_handle(handle)))
    }

    pub fn generate_irradiance_map(
        &self,
        resource_manager: &GpuResourceManager,
        src: &HdrCubeTexture,
        dst_size: u32,
    ) -> anyhow::Result<HdrCubeTexture> {
        let src_handle = &src.lazy_init(resource_manager)?[0];
        let src = src_handle.get_texture().unwrap();
        let src_view = src.create_view(&wgpu::TextureViewDescriptor {
            label: Some("HDR Irradiance Source Texture View"),
            format: Some(HdrCubeTexture::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let dst_buf = resource_manager.create_texture(
            dst_size,
            dst_size,
            HdrCubeTexture::FORMAT,
            wgpu::TextureDimension::D2,
            6,
            wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            Some("HDR Irradiance Destination Texture"),
        );

        let dst_view = match &*dst_buf {
            GpuResource::Texture { texture, .. } => {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("HDR Irradiance Destination Texture View"),
                    format: Some(HdrCubeTexture::FORMAT),
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    aspect: wgpu::TextureAspect::All,
                    array_layer_count: Some(6),
                    ..Default::default()
                })
            }
            _ => unreachable!(),
        };

        let sampler = resource_manager
            .device()
            .create_sampler(&wgpu::SamplerDescriptor {
                label: Some("HDR Irradiance Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });

        let bind_group = resource_manager
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("HDR Irradiance Bind Group"),
                layout: &self.irradiance_layout,
                entries: &[
                    // src
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&src_view),
                    },
                    // sampler
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });

        let mut encoder =
            resource_manager
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("HDR Irradiance Encoder"),
                });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HDR Irradiance Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &dst_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(&self.irradiance_pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.set_bind_group(1, &self.irradiance_transforms_bind_group, &[]);
            rpass.draw(0..6, 0..1);
        }

        resource_manager
            .queue()
            .submit(std::iter::once(encoder.finish()));

        let handle = resource_manager.insert_resource(dst_buf);

        let handle = LazyGpuHandle::new_ready(handle);

        Ok(HdrCubeTexture::from_texture(Texture::from_handle(handle)))
    }
}
