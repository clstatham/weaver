use std::{num::NonZeroU32, path::Path, sync::Arc};

use weaver_app::{plugin::Plugin, App};
use weaver_ecs::prelude::{Resource, World};
use weaver_renderer::{
    bind_group::BindGroupLayoutCache,
    extract::{RenderResource, RenderResourceDependencyPlugin},
    prelude::wgpu,
    shader::Shader,
    texture::texture_format,
    WgpuDevice, WgpuQueue,
};
use weaver_util::prelude::Result;
use wgpu::util::DeviceExt;

use crate::prelude::GpuSkybox;

#[derive(Clone, Resource)]
pub(crate) struct GpuSkyboxIrradiance {
    pub texture: Arc<wgpu::Texture>,
    pub cube_view: Arc<wgpu::TextureView>,
    pub sampler: Arc<wgpu::Sampler>,

    views: Arc<wgpu::Buffer>,
    projection: Arc<wgpu::Buffer>,
}

impl GpuSkyboxIrradiance {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        skybox_view: &wgpu::TextureView,
        skybox_sampler: &wgpu::Sampler,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> Self {
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
            label: Some("Skybox Irradiance Views"),
            contents: bytemuck::cast_slice(&views),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let projection = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skybox Irradiance Projection"),
            contents: bytemuck::cast_slice(&[glam::Mat4::perspective_rh(
                std::f32::consts::FRAC_PI_2,
                1.0,
                0.1,
                10.0,
            )]),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let irradiance_transforms_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Skybox Irradiance Transforms Bind Group Layout"),
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

        let irradiance_transforms_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Skybox Irradiance Transforms Bind Group"),
                layout: &irradiance_transforms_layout,
                entries: &[
                    // views
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: views.as_entire_binding(),
                    },
                    // projection
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: projection.as_entire_binding(),
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Skybox Irradiance Pipeline Layout"),
            bind_group_layouts: &[
                &bind_group_layout_cache.get_or_create::<GpuSkybox>(device),
                &irradiance_transforms_layout,
            ],
            push_constant_ranges: &[],
        });

        let shader = Shader::new(Path::new("assets/shaders/hdr_irradiance.wgsl"))
            .create_shader_module(device);

        let irradiance_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HDR Loader Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "irradiance_vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "irradiance_fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture_format::HDR_FORMAT,
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

        let dst_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Skybox Irradiance Texture"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format::HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[texture_format::HDR_FORMAT],
        });

        let dst_view = dst_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Skybox Irradiance Texture View"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            format: Some(texture_format::HDR_FORMAT),
            array_layer_count: Some(6),
            ..Default::default()
        });

        let src_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skybox Irradiance Source Bind Group"),
            layout: bind_group_layout_cache.get::<GpuSkybox>().unwrap(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(skybox_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(skybox_sampler),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Skybox Irradiance Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Skybox Irradiance Render Pass"),
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

            render_pass.set_pipeline(&irradiance_pipeline);
            render_pass.set_bind_group(0, &src_bind_group, &[]);
            render_pass.set_bind_group(1, &irradiance_transforms_bind_group, &[]);
            render_pass.draw(0..6, 0..1);
        }

        queue.submit(std::iter::once(encoder.finish()));

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Skybox Irradiance Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let cube_view = dst_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Skybox Irradiance Cube View"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            format: Some(texture_format::HDR_FORMAT),
            ..Default::default()
        });

        Self {
            texture: Arc::new(dst_texture),
            cube_view: Arc::new(cube_view),
            sampler: Arc::new(sampler),
            views: Arc::new(views),
            projection: Arc::new(projection),
        }
    }
}

impl RenderResource for GpuSkyboxIrradiance {
    type UpdateQuery = ();

    fn extract_render_resource(_main_world: &mut World, render_world: &mut World) -> Option<Self>
    where
        Self: Sized,
    {
        let device = render_world.get_resource::<WgpuDevice>().unwrap();
        let queue = render_world.get_resource::<WgpuQueue>().unwrap();
        let skybox = render_world.get_resource::<GpuSkybox>().unwrap();
        let mut bind_group_layout_cache = render_world
            .get_resource_mut::<BindGroupLayoutCache>()
            .unwrap();

        Some(Self::new(
            &device,
            &queue,
            &skybox.cube_view,
            &skybox.sampler,
            &mut bind_group_layout_cache,
        ))
    }

    fn update_render_resource(
        &mut self,
        _main_world: &mut World,
        _render_world: &mut World,
    ) -> Result<()> {
        Ok(())
    }
}

pub struct SkyboxIrradiancePlugin;

impl Plugin for SkyboxIrradiancePlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(RenderResourceDependencyPlugin::<
            GpuSkyboxIrradiance,
            GpuSkybox,
        >::default())?;

        Ok(())
    }
}
