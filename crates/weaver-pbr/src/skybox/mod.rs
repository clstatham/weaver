use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

use image::codecs::hdr::HdrDecoder;
use weaver_app::plugin::Plugin;
use weaver_ecs::{
    component::Res,
    prelude::{QueryFetchItem, Resource, SystemParamItem, World},
    world::FromWorld,
};
use weaver_renderer::{
    bind_group::{
        BindGroup, BindGroupLayout, BindGroupLayoutCache, CreateBindGroup, ResourceBindGroupPlugin,
    },
    camera::{CameraBindGroup, ViewTarget},
    extract::{ExtractResource, ExtractResourcePlugin},
    graph::{RenderCtx, RenderGraphApp, RenderGraphCtx, ViewNode, ViewNodeRunner},
    hdr::HdrRenderTarget,
    pipeline::{
        ComputePipelineCache, ComputePipelinePlugin, CreateComputePipeline, CreateRenderPipeline,
        RenderPipeline, RenderPipelineCache, RenderPipelineLayout, RenderPipelinePlugin,
    },
    prelude::{wgpu, ComputePipeline, ComputePipelineLayout},
    shader::Shader,
    texture::{texture_format, GpuTexture},
    InitRenderResources, RenderLabel, WgpuDevice, WgpuQueue,
};
use weaver_util::Result;

pub mod irradiance;

pub const SKYBOX_CUBEMAP_SIZE: u32 = 1024;

#[derive(Resource, Clone)]
pub struct Skybox {
    pub path: PathBuf,
    pub diffuse_path: PathBuf,
    pub specular_path: PathBuf,
    pub brdf_lut_path: PathBuf,
}

impl Skybox {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let file_stem = path.file_stem().unwrap().to_str().unwrap().to_owned();
        let diffuse_path = path
            .with_file_name(file_stem.clone() + "_diffuse")
            .with_extension("ktx2");
        let specular_path = path
            .with_file_name(file_stem.clone() + "_specular")
            .with_extension("ktx2");
        let brdf_lut_path = path
            .with_file_name(file_stem.clone() + "_LUT")
            .with_extension("png");

        Self {
            path,
            diffuse_path,
            specular_path,
            brdf_lut_path,
        }
    }
}

impl ExtractResource for Skybox {
    type Source = Skybox;

    fn extract_render_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

pub(crate) struct GpuSkyboxSrc {
    src_texture: GpuTexture,
}

impl CreateBindGroup for GpuSkyboxSrc {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Skybox Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            }],
        })
    }

    fn create_bind_group(&self, device: &wgpu::Device, layout: &BindGroupLayout) -> wgpu::BindGroup
    where
        Self: Sized,
    {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skybox Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&self.src_texture.view),
            }],
        })
    }
}

pub(crate) struct GpuSkyboxDst {
    dst_texture: GpuTexture,
}

#[derive(Clone, Resource)]
pub(crate) struct GpuSkybox {
    #[allow(unused)]
    pub(crate) texture: GpuTexture,
    pub(crate) cube_view: Arc<wgpu::TextureView>,
    pub(crate) sampler: Arc<wgpu::Sampler>,
}

impl FromWorld for GpuSkybox {
    fn from_world(world: &mut World) -> Self {
        let (device, queue, skybox, mut pipeline_cache, mut bind_group_layout_cache) = world
            .get_many_resources_mut::<(
                WgpuDevice,
                WgpuQueue,
                Skybox,
                ComputePipelineCache,
                BindGroupLayoutCache,
            )>();
        Self::new(
            &skybox,
            &device,
            &queue,
            &mut pipeline_cache,
            &mut bind_group_layout_cache,
        )
    }
}

impl GpuSkybox {
    pub fn new(
        skybox: &Skybox,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipeline_cache: &mut ComputePipelineCache,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> Self {
        let mut file = File::open(&skybox.path).unwrap();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        drop(file);

        let hdr_decoder = HdrDecoder::new(buf.as_slice()).unwrap();
        let meta = hdr_decoder.metadata();
        let mut pixels = vec![[0.0, 0.0, 0.0, 0.0]; (meta.width * meta.height) as usize];
        hdr_decoder
            .read_image_transform(
                |p| {
                    let rgb = p.to_hdr();
                    [rgb[0], rgb[1], rgb[2], 1.0]
                },
                &mut pixels,
            )
            .unwrap();

        let src = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Skybox Source Texture"),
            size: wgpu::Extent3d {
                width: meta.width,
                height: meta.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format::HDR_CUBE_FORMAT,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let src_view = src.create_view(&wgpu::TextureViewDescriptor::default());
        queue.write_texture(
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

        let dst = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Skybox Destination Texture"),
            size: wgpu::Extent3d {
                width: SKYBOX_CUBEMAP_SIZE,
                height: SKYBOX_CUBEMAP_SIZE,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format::HDR_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let dst_view = dst.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(6),
            ..Default::default()
        });

        let skybox_src = GpuSkyboxSrc {
            src_texture: GpuTexture {
                texture: Arc::new(src),
                view: Arc::new(src_view),
            },
        };

        let src_bind_group = skybox_src.create_bind_group(
            device,
            &BindGroupLayout::get_or_create::<GpuSkyboxSrc>(device, bind_group_layout_cache),
        );

        let dst = GpuSkyboxDst {
            dst_texture: GpuTexture {
                texture: Arc::new(dst),
                view: Arc::new(dst_view),
            },
        };

        let dst_bind_group = dst.create_bind_group(
            device,
            &BindGroupLayout::get_or_create::<GpuSkyboxDst>(device, bind_group_layout_cache),
        );

        let pipeline =
            pipeline_cache.get_or_create_pipeline::<GpuSkybox>(device, bind_group_layout_cache);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Skybox Load Encoder"),
        });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Skybox Load Pass"),
                timestamp_writes: None,
            });
            let num_workgroups = (SKYBOX_CUBEMAP_SIZE + 15) / 16;
            cpass.set_pipeline(&pipeline);
            cpass.set_bind_group(0, &src_bind_group, &[]);
            cpass.set_bind_group(1, &dst_bind_group, &[]);
            cpass.dispatch_workgroups(num_workgroups, num_workgroups, 6);
        }
        queue.submit(Some(encoder.finish()));

        let GpuSkyboxDst { dst_texture } = dst;

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Skybox Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let cube_view = dst_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                array_layer_count: Some(6),
                ..Default::default()
            });

        Self {
            texture: dst_texture,
            cube_view: Arc::new(cube_view),
            sampler: Arc::new(sampler),
        }
    }
}

impl CreateComputePipeline for GpuSkybox {
    fn create_compute_pipeline_layout(
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> ComputePipelineLayout
    where
        Self: Sized,
    {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Skybox Pipeline Layout"),
            bind_group_layouts: &[
                &bind_group_layout_cache.get_or_create::<GpuSkyboxSrc>(device),
                &bind_group_layout_cache.get_or_create::<GpuSkyboxDst>(device),
            ],
            push_constant_ranges: &[],
        });

        ComputePipelineLayout::new(layout)
    }

    fn create_compute_pipeline(
        device: &wgpu::Device,
        cached_layout: &wgpu::PipelineLayout,
    ) -> ComputePipeline
    where
        Self: Sized,
    {
        let module = Shader::new(Path::new("assets/shaders/skybox_loader.wgsl"))
            .create_shader_module(device);

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Skybox Compute Pipeline"),
            layout: Some(cached_layout),
            module: &module,
            entry_point: "load",
        });

        ComputePipeline::new(pipeline)
    }
}

impl CreateBindGroup for GpuSkyboxDst {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Skybox Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: texture_format::HDR_FORMAT,
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                },
                count: None,
            }],
        })
    }

    fn create_bind_group(&self, device: &wgpu::Device, layout: &BindGroupLayout) -> wgpu::BindGroup
    where
        Self: Sized,
    {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skybox Aux Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&self.dst_texture.view),
            }],
        })
    }
}

impl CreateBindGroup for GpuSkybox {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Skybox Bind Group Layout"),
            entries: &[
                // skybox texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::Cube,
                    },
                    count: None,
                },
                // skybox sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    fn create_bind_group(&self, device: &wgpu::Device, layout: &BindGroupLayout) -> wgpu::BindGroup
    where
        Self: Sized,
    {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skybox Bind Group"),
            layout,
            entries: &[
                // skybox texture
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.cube_view),
                },
                // skybox sampler
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SkyboxNodeLabel;
impl RenderLabel for SkyboxNodeLabel {}

#[derive(Default)]
pub(crate) struct SkyboxNode;

impl CreateRenderPipeline for SkyboxNode {
    fn create_render_pipeline_layout(
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> RenderPipelineLayout
    where
        Self: Sized,
    {
        RenderPipelineLayout::new(
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Skybox Pipeline Layout"),
                bind_group_layouts: &[
                    &bind_group_layout_cache.get_or_create::<GpuSkybox>(device),
                    &bind_group_layout_cache.get_or_create::<CameraBindGroup>(device),
                ],
                push_constant_ranges: &[],
            }),
        )
    }

    fn create_render_pipeline(
        device: &wgpu::Device,
        cached_layout: &wgpu::PipelineLayout,
    ) -> RenderPipeline
    where
        Self: Sized,
    {
        let module = Shader::new(Path::new("assets/shaders/sky.wgsl")).create_shader_module(device);

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skybox Render Pipeline"),
            layout: Some(cached_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: "fs_main",
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture_format::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        RenderPipeline::new(pipeline)
    }
}

impl ViewNode for SkyboxNode {
    type Param = (
        Res<'static, RenderPipelineCache>,
        Res<'static, HdrRenderTarget>,
        Res<'static, BindGroup<GpuSkybox>>,
    );

    type ViewQueryFetch = (&'static ViewTarget, &'static BindGroup<CameraBindGroup>);

    type ViewQueryFilter = ();

    fn run(
        &self,
        _render_world: &World,
        _graph_ctx: &mut RenderGraphCtx,
        render_ctx: &mut RenderCtx,
        (render_pipeline_cache, hdr_target, skybox_bind_group): &SystemParamItem<Self::Param>,
        (view_target, camera_bind_group): &QueryFetchItem<Self::ViewQueryFetch>,
    ) -> Result<()> {
        let skybox_pipeline = render_pipeline_cache
            .get_pipeline_for::<SkyboxNode>()
            .unwrap();

        {
            let mut rpass =
                render_ctx
                    .command_encoder()
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Skybox Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: hdr_target.color_target(),
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &view_target.depth_target,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });

            rpass.set_pipeline(skybox_pipeline);
            rpass.set_bind_group(0, skybox_bind_group, &[]);
            rpass.set_bind_group(1, camera_bind_group, &[]);
            rpass.draw(0..3, 0..1);
        }

        Ok(())
    }
}

pub struct SkyboxPlugin;

impl Plugin for SkyboxPlugin {
    fn build(&self, render_app: &mut weaver_app::App) -> Result<()> {
        render_app.add_plugin(ExtractResourcePlugin::<Skybox>::default())?;
        render_app.add_plugin(ComputePipelinePlugin::<GpuSkybox>::default())?;
        render_app.add_plugin(ResourceBindGroupPlugin::<GpuSkybox>::default())?;

        render_app.add_system(init_gpu_skybox, InitRenderResources);

        Ok(())
    }
}

pub fn init_gpu_skybox(world: &mut World) {
    if !world.has_resource::<GpuSkybox>() && world.has_resource::<Skybox>() {
        world.init_resource::<GpuSkybox>();
    }
}

pub struct SkyboxNodePlugin;

impl Plugin for SkyboxNodePlugin {
    fn build(&self, render_app: &mut weaver_app::App) -> Result<()> {
        render_app.add_plugin(RenderPipelinePlugin::<SkyboxNode>::default())?;
        render_app.add_render_main_graph_node::<ViewNodeRunner<SkyboxNode>>(SkyboxNodeLabel);

        Ok(())
    }
}
