use encase::ShaderType;
use weaver_asset::{Assets, Handle};
use weaver_core::prelude::{Mat4, Transform};
use weaver_ecs::prelude::*;
use weaver_renderer::{
    buffer::GpuBufferVec,
    camera::CameraBindGroup,
    extract::Extract,
    hdr::HdrRenderTarget,
    pipeline::RenderPipelineCache,
    prelude::{
        wgpu::{self, ShaderStages},
        BindGroup, BindGroupLayoutCache, CreateBindGroup, CreateRenderPipeline,
        RenderPipelineLayout,
    },
    resources::ActiveCommandEncoder,
    texture::{texture_format, GpuTexture},
    WgpuDevice, WgpuQueue,
};

use crate::{
    geometry::Sphere,
    light::RaytracingLightingInformation,
    material::{Material, MaterialUniform},
};

pub struct RaytracingOutput {
    pub hdr_texture: GpuTexture,
}

impl CreateBindGroup for RaytracingOutput {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: texture_format::HDR_FORMAT,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            }],
            label: Some("Raytracing Output Bind Group Layout"),
        })
    }

    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        cached_layout: &weaver_renderer::prelude::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: cached_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&self.hdr_texture.view),
            }],
            label: Some("Raytracing Output Bind Group"),
        })
    }
}

pub async fn init_raytracing_output(
    commands: Commands,
    device: Res<WgpuDevice>,
    hdr_target: Res<HdrRenderTarget>,
    output: Option<ResMut<RaytracingOutput>>,
    mut cache: ResMut<BindGroupLayoutCache>,
) {
    if output.is_none() {
        drop(output);

        let width = hdr_target.texture.texture.width();
        let height = hdr_target.texture.texture.height();

        let hdr_texture = GpuTexture::new(
            &device,
            Some("Raytracing Output Texture"),
            width,
            height,
            texture_format::HDR_FORMAT,
            wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        );

        let output = RaytracingOutput { hdr_texture };

        if !commands.has_resource::<BindGroup<RaytracingOutput>>().await {
            let bind_group = BindGroup::new(&device, &output, &mut cache);
            commands.insert_resource(bind_group).await;
        }

        commands.insert_resource(output).await;
    }
}

#[derive(Debug, Clone, Copy, ShaderType)]
pub struct GpuObjectRaytracingUniform {
    pub model_transform: Mat4,
    pub material: MaterialUniform,
    pub radius: f32,
}

pub struct GpuObjectRaytracingBuffer {
    pub buffer: GpuBufferVec<GpuObjectRaytracingUniform>,
}

impl ConstructFromWorld for GpuObjectRaytracingBuffer {
    fn from_world(world: &World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();
        let queue = world.get_resource::<WgpuQueue>().unwrap();
        let mut buffer =
            GpuBufferVec::new(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        buffer.reserve(1, &device);
        buffer.enqueue_update(&device, &queue);
        Self { buffer }
    }
}

impl CreateBindGroup for GpuObjectRaytracingBuffer {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    min_binding_size: None,
                    has_dynamic_offset: false,
                },
                count: None,
            }],
            label: Some("GpuObjectRaytracingBuffer"),
        })
    }

    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        cached_layout: &weaver_renderer::prelude::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: cached_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.buffer.binding().unwrap(),
            }],
            label: Some("GpuObjectRaytracingBuffer"),
        })
    }
}

pub async fn init_gpu_object_raytracing_buffer(commands: Commands) {
    if !commands.has_resource::<GpuObjectRaytracingBuffer>().await {
        commands.init_resource::<GpuObjectRaytracingBuffer>().await;
    }
}

pub async fn extract_gpu_object_raytracing_buffer(
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
    mut buffer: ResMut<GpuObjectRaytracingBuffer>,
    mut query: Extract<Query<(&Transform, &Sphere, &Handle<Material>)>>,
    mut material_assets: Extract<ResMut<Assets<Material>>>,
) {
    buffer.buffer.clear();

    for (transform, sphere, mat_handle) in query.iter() {
        let material = material_assets.get(*mat_handle).unwrap();
        let uniform = GpuObjectRaytracingUniform {
            model_transform: transform.matrix(),
            material: MaterialUniform {
                diffuse: material.diffuse,
            },
            radius: sphere.radius,
        };

        buffer.buffer.push(uniform);
    }

    buffer.buffer.enqueue_update(&device, &queue);
    queue.submit(None);
    device.poll(wgpu::Maintain::Wait);
}

pub struct RaytracingRenderPipeline;

impl CreateRenderPipeline for RaytracingRenderPipeline {
    fn create_render_pipeline_layout(
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> RenderPipelineLayout
    where
        Self: Sized,
    {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Raytracing Fragment Pipeline Layout"),
            bind_group_layouts: &[
                &bind_group_layout_cache.get_or_create::<CameraBindGroup>(device),
                &bind_group_layout_cache.get_or_create::<RaytracingLightingInformation>(device),
                &bind_group_layout_cache.get_or_create::<GpuObjectRaytracingBuffer>(device),
            ],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::FRAGMENT,
                range: 0..(std::mem::size_of::<[f32; 3]>() as u32),
            }],
        });

        RenderPipelineLayout::new(layout)
    }

    fn create_render_pipeline(
        device: &wgpu::Device,
        cached_layout: &wgpu::PipelineLayout,
    ) -> weaver_renderer::prelude::RenderPipeline
    where
        Self: Sized,
    {
        let shader = wgpu::include_wgsl!("../assets/raytracing.wgsl");
        let shader = device.create_shader_module(shader);

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Raytracing Pipeline"),
            layout: Some(cached_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "raytracing_vs_main",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "raytracing_fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture_format::HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            cache: None,
            multiview: None,
        });

        weaver_renderer::prelude::RenderPipeline::new(pipeline)
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn render_raytracing(
    pipeline_cache: Res<RenderPipelineCache>,
    object_bind_group: Res<BindGroup<GpuObjectRaytracingBuffer>>,
    lighting_bind_group: Res<BindGroup<RaytracingLightingInformation>>,
    mut camera_bind_group: Query<&BindGroup<CameraBindGroup>>,
    hdr_target: Res<HdrRenderTarget>,
    mut encoder: ResMut<ActiveCommandEncoder>,
) {
    let pipeline = pipeline_cache
        .get_pipeline_for::<RaytracingRenderPipeline>()
        .unwrap();

    let camera_bind_group = camera_bind_group.iter().next().unwrap();

    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Raytracing Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &hdr_target.texture.view,
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

    rpass.set_pipeline(pipeline);
    rpass.set_bind_group(0, &camera_bind_group, &[]);
    rpass.set_bind_group(1, &lighting_bind_group, &[]);
    rpass.set_bind_group(2, &object_bind_group, &[]);

    rpass.set_push_constants(
        ShaderStages::FRAGMENT,
        0,
        bytemuck::cast_slice(&[
            rand::random::<f32>(),
            rand::random::<f32>(),
            rand::random::<f32>(),
        ]),
    );

    rpass.draw(0..3, 0..1);
}
