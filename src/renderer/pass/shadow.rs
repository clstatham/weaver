use std::num::NonZeroU32;

use super::Pass;
use crate::{
    app::asset_server::AssetId,
    core::{
        camera::{Camera, CameraUniform},
        light::{DirectionalLight, DirectionalLightUniform, PointLight, PointLightUniform},
        mesh::{Mesh, Vertex, MAX_MESHES},
        physics::{RapierContext, RigidBody},
        texture::Texture,
        transform::Transform,
    },
    ecs::{Query, World},
    include_shader,
};
use rustc_hash::FxHashMap;

const SHADOW_DEPTH_TEXTURE_SIZE: u32 = 1024;

struct UniqueMesh {
    mesh: Mesh,
    transforms: Vec<glam::Mat4>,
}

impl UniqueMesh {
    fn gather(world: &World) -> FxHashMap<AssetId, Self> {
        let mut meshes = FxHashMap::default();

        // gather all the meshes with transforms
        let query = Query::<(&Mesh, &Transform)>::new(world);
        for (mesh, transform) in query.iter() {
            let mesh = mesh.clone();
            let mesh_id = mesh.asset_id();
            let unique_mesh = meshes.entry(mesh_id).or_insert(Self {
                mesh,
                transforms: Vec::new(),
            });
            unique_mesh.transforms.push(transform.matrix);
        }

        // gather all the meshes with rigid bodies
        if let Ok(mut ctx) = world.write_resource::<RapierContext>() {
            let query = Query::<(&Mesh, &mut RigidBody)>::new(world);
            for (mesh, mut rigid_body) in query.iter() {
                let mesh = mesh.clone();
                let mesh_id = mesh.asset_id();
                let unique_mesh = meshes.entry(mesh_id).or_insert(Self {
                    mesh,
                    transforms: Vec::new(),
                });
                unique_mesh
                    .transforms
                    .push(rigid_body.get_transform(&mut ctx).matrix);
            }
        }

        meshes
    }
}

#[allow(dead_code)]
pub struct ShadowRenderPass {
    enabled: bool,

    // the first stage creates the shadow map
    shadow_map_pipeline_layout: wgpu::PipelineLayout,
    shadow_map_pipeline: wgpu::RenderPipeline,
    shadow_map_bind_group_layout: wgpu::BindGroupLayout,
    shadow_map_bind_group: wgpu::BindGroup,

    // the second stage creates the shadow cube map
    shadow_cube_map_pipeline_layout: wgpu::PipelineLayout,
    shadow_cube_map_pipeline: wgpu::RenderPipeline,
    shadow_cube_map_bind_group_layout: wgpu::BindGroupLayout,
    shadow_cube_map_bind_group: wgpu::BindGroup,

    // the third stage overlays the shadow map on the scene
    shadow_overlay_pipeline_layout: wgpu::PipelineLayout,
    shadow_overlay_pipeline: wgpu::RenderPipeline,
    shadow_overlay_bind_group_layout: wgpu::BindGroupLayout,
    shadow_overlay_bind_group: wgpu::BindGroup,

    // the fourth stage overlays the shadow cube map on the scene
    shadow_cube_overlay_pipeline_layout: wgpu::PipelineLayout,
    shadow_cube_overlay_pipeline: wgpu::RenderPipeline,
    shadow_cube_overlay_bind_group_layout: wgpu::BindGroupLayout,
    shadow_cube_overlay_bind_group: wgpu::BindGroup,

    // shadow map texture
    shadow_depth_texture: Texture,
    // shadow cube map texture (for point lights)
    shadow_cube_texture: Texture,
    // shadow cube map individual face views
    shadow_cube_d2array_view: wgpu::TextureView,
    // shadow cube map depth target cubemap
    shadow_cube_depth_target: Texture,
    // shadow cube map depth target cubemap views
    shadow_cube_depth_target_d2array_view: wgpu::TextureView,

    // miscellaneous buffers used in bind groups
    model_transform_buffer: wgpu::Buffer,
    directional_light_buffer: wgpu::Buffer,
    point_light_buffer: wgpu::Buffer,
    point_light_view_transform_buffer: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,
}

impl ShadowRenderPass {
    pub fn new(
        device: &wgpu::Device,
        color_sampler: &wgpu::Sampler,
        depth_sampler: &wgpu::Sampler,
    ) -> Self {
        let shadow_depth_texture = Texture::new_lazy(
            SHADOW_DEPTH_TEXTURE_SIZE,
            SHADOW_DEPTH_TEXTURE_SIZE,
            Some("Shadow Depth Texture"),
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            Texture::DEPTH_FORMAT,
            wgpu::TextureDimension::D2,
            wgpu::TextureViewDimension::D2,
            1,
        );

        let shadow_cube_texture = Texture::new_lazy(
            SHADOW_DEPTH_TEXTURE_SIZE,
            SHADOW_DEPTH_TEXTURE_SIZE,
            Some("Shadow Cube Texture"),
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            wgpu::TextureFormat::R32Float,
            wgpu::TextureDimension::D2,
            wgpu::TextureViewDimension::D2,
            6,
        );

        let shadow_cube_depth_target =
            Texture::new_depth_cubemap(device, SHADOW_DEPTH_TEXTURE_SIZE);

        let model_transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Model Transform Buffer"),
            size: (std::mem::size_of::<glam::Mat4>() * MAX_MESHES) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let directional_light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Directional Light Buffer"),
            size: std::mem::size_of::<DirectionalLightUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let point_light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Point Light Buffer"),
            size: std::mem::size_of::<PointLightUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let point_light_view_transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Point Light View Transform Buffer"),
            size: std::mem::size_of::<glam::Mat4>() as u64 * 6,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Camera Buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // first stage: create the shadow map

        let shadow_map_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow Map Bind Group Layout"),
                entries: &[
                    // model transform
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
                    // directional light
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let shadow_map_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Map Bind Group"),
            layout: &shadow_map_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: model_transform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: directional_light_buffer.as_entire_binding(),
                },
            ],
        });

        let shadow_map_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow Map Pipeline Layout"),
                bind_group_layouts: &[&shadow_map_bind_group_layout],
                push_constant_ranges: &[],
            });

        let shadow_map_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Map Pipeline"),
            layout: Some(&shadow_map_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Shadow Map Vertex Shader"),
                    source: wgpu::ShaderSource::Wgsl(include_shader!("shadow_map.wgsl").into()),
                }),
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // second stage: create the shadow cube map

        let shadow_cube_map_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow Cube Map Bind Group Layout"),
                entries: &[
                    // model transform
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
                    // point light
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // point light view transform
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
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

        let shadow_cube_map_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Cube Map Bind Group"),
            layout: &shadow_cube_map_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: model_transform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: point_light_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: point_light_view_transform_buffer.as_entire_binding(),
                },
            ],
        });

        let shadow_cube_map_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow Cube Map Pipeline Layout"),
                bind_group_layouts: &[&shadow_cube_map_bind_group_layout],
                push_constant_ranges: &[],
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Cube Map Shader"),
            source: wgpu::ShaderSource::Wgsl(include_shader!("shadow_cubemap.wgsl").into()),
        });

        let shadow_cube_map_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Shadow Cube Map Pipeline"),
                layout: Some(&shadow_cube_map_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::R32Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                // fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Texture::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: Some(NonZeroU32::new(6).unwrap()),
            });

        // third stage: overlay the shadow map on the scene

        let shadow_overlay_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow Overlay Bind Group Layout"),
                entries: &[
                    // shadow map
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // shadow map sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                        count: None,
                    },
                    // camera uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // directional light uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // model transform
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
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

        let shadow_overlay_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Overlay Bind Group"),
            layout: &shadow_overlay_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(shadow_depth_texture.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(depth_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: directional_light_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: model_transform_buffer.as_entire_binding(),
                },
            ],
        });

        let shadow_overlay_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow Overlay Pipeline Layout"),
                bind_group_layouts: &[&shadow_overlay_bind_group_layout],
                push_constant_ranges: &[],
            });

        let shadow_overlay_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Shadow Overlay Pipeline"),
                layout: Some(&shadow_overlay_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("Shadow Overlay Vertex Shader"),
                        source: wgpu::ShaderSource::Wgsl(
                            include_shader!("shadow_overlay.wgsl").into(),
                        ),
                    }),
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("Shadow Overlay Fragment Shader"),
                        source: wgpu::ShaderSource::Wgsl(
                            include_shader!("shadow_overlay.wgsl").into(),
                        ),
                    }),
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: Texture::WINDOW_FORMAT,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Texture::DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        // fourth stage: overlay the shadow cube map on the scene

        let shadow_cube_overlay_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow Cube Overlay Bind Group Layout"),
                entries: &[
                    // shadow cubemap
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::Cube,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // shadow cubemap sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                    // camera uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // point light uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // model transform
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
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

        let shadow_cube_overlay_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Cube Overlay Bind Group"),
            layout: &shadow_cube_overlay_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(shadow_cube_texture.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(color_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: point_light_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: model_transform_buffer.as_entire_binding(),
                },
            ],
        });

        let shadow_cube_overlay_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow Cube Overlay Pipeline Layout"),
                bind_group_layouts: &[&shadow_cube_overlay_bind_group_layout],
                push_constant_ranges: &[],
            });

        let shadow_cube_overlay_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Shadow Cube Overlay Pipeline"),
                layout: Some(&shadow_cube_overlay_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("Shadow Cube Overlay Vertex Shader"),
                        source: wgpu::ShaderSource::Wgsl(
                            include_shader!("shadow_cubemap_overlay.wgsl").into(),
                        ),
                    }),
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("Shadow Cube Overlay Fragment Shader"),
                        source: wgpu::ShaderSource::Wgsl(
                            include_shader!("shadow_cubemap_overlay.wgsl").into(),
                        ),
                    }),
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: Texture::WINDOW_FORMAT,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Texture::DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        Self {
            enabled: true,
            shadow_map_pipeline_layout,
            shadow_map_pipeline,
            shadow_map_bind_group_layout,
            shadow_map_bind_group,
            shadow_cube_map_pipeline_layout,
            shadow_cube_map_pipeline,
            shadow_cube_map_bind_group_layout,
            shadow_cube_map_bind_group,
            shadow_overlay_pipeline_layout,
            shadow_overlay_pipeline,
            shadow_overlay_bind_group_layout,
            shadow_overlay_bind_group,
            shadow_cube_overlay_pipeline_layout,
            shadow_cube_overlay_pipeline,
            shadow_cube_overlay_bind_group_layout,
            shadow_cube_overlay_bind_group,
            shadow_depth_texture,
            shadow_cube_texture,
            shadow_cube_d2array_view,
            shadow_cube_depth_target,
            shadow_cube_depth_target_d2array_view,
            model_transform_buffer,
            directional_light_buffer,
            point_light_buffer,
            point_light_view_transform_buffer,
            camera_buffer,
        }
    }

    fn render_shadow_map(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        directional_light: &DirectionalLight,
        camera: &Camera,
        unique_meshes: &FxHashMap<AssetId, UniqueMesh>,
    ) -> anyhow::Result<()> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Shadow Initial Encoder"),
        });

        // clear the shadow map texture
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Render Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: self.shadow_depth_texture.view(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        let camera_uniform = CameraUniform::from(camera);

        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );

        let directional_light_uniform = DirectionalLightUniform::from(directional_light);

        queue.write_buffer(
            &self.directional_light_buffer,
            0,
            bytemuck::cast_slice(&[directional_light_uniform]),
        );

        queue.submit(std::iter::once(encoder.finish()));

        for unique_mesh in unique_meshes.values() {
            let UniqueMesh { mesh, transforms } = unique_mesh;

            let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Buffer Write Encoder"),
            });

            queue.write_buffer(
                &self.model_transform_buffer,
                0,
                bytemuck::cast_slice(transforms.as_slice()),
            );

            queue.submit(std::iter::once(encoder.finish()));

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Render Pass Encoder"),
            });

            // build the shadow map
            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Shadow Render Pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: self.shadow_depth_texture.view(),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.shadow_map_pipeline);
                render_pass.set_bind_group(0, &self.shadow_map_bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(
                    0..mesh.num_indices() as u32,
                    0,
                    0..transforms.len() as u32,
                );
            }

            queue.submit(std::iter::once(encoder.finish()));
        }

        Ok(())
    }

    fn overlay_shadow_map(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_target: &Texture,
        depth_target: &Texture,
        camera: &Camera,
        unique_meshes: &FxHashMap<AssetId, UniqueMesh>,
    ) -> anyhow::Result<()> {
        let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Shadow Overlay Initial Encoder"),
        });

        let camera_uniform = CameraUniform::from(camera);

        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );

        queue.submit(std::iter::once(encoder.finish()));

        // overlay the built shadow map on the screen
        for unique_mesh in unique_meshes.values() {
            let UniqueMesh { mesh, transforms } = unique_mesh;
            let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Overlay Buffer Write Encoder"),
            });

            queue.write_buffer(
                &self.model_transform_buffer,
                0,
                bytemuck::cast_slice(transforms.as_slice()),
            );

            queue.submit(std::iter::once(encoder.finish()));

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Overlay Render Pass Encoder"),
            });

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Shadow Overlay Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color_target.view(),
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_target.view(),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.shadow_overlay_pipeline);
                render_pass.set_bind_group(0, &self.shadow_overlay_bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(
                    0..mesh.num_indices() as u32,
                    0,
                    0..transforms.len() as u32,
                );
            }
            queue.submit(std::iter::once(encoder.finish()));
        }

        Ok(())
    }

    fn render_cube_map(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        point_light: &PointLight,
        unique_meshes: &FxHashMap<AssetId, UniqueMesh>,
    ) -> anyhow::Result<()> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Shadow Cube Map Encoder"),
        });

        // clear the shadow cubemap texture
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Cube Map Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.shadow_cube_d2array_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::MAX,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_cube_depth_target_d2array_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        let point_light_uniform = PointLightUniform::from(point_light);

        queue.write_buffer(
            &self.point_light_buffer,
            0,
            bytemuck::cast_slice(&[point_light_uniform]),
        );

        queue.submit(std::iter::once(encoder.finish()));

        for unique_mesh in unique_meshes.values() {
            let UniqueMesh { mesh, transforms } = unique_mesh;

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Cube Map Render Encoder"),
            });

            queue.write_buffer(
                &self.model_transform_buffer,
                0,
                bytemuck::cast_slice(transforms.as_slice()),
            );

            let mut light_views = Vec::new();

            for i in 0..6 {
                let view_transform = match i {
                    // right
                    0 => point_light.view_transform_in_direction(glam::Vec3::X, glam::Vec3::Y),
                    // left
                    1 => point_light.view_transform_in_direction(-glam::Vec3::X, glam::Vec3::Y),
                    // top
                    2 => point_light.view_transform_in_direction(glam::Vec3::Y, -glam::Vec3::Z),
                    // bottom
                    3 => point_light.view_transform_in_direction(-glam::Vec3::Y, glam::Vec3::Z),
                    // front
                    4 => point_light.view_transform_in_direction(glam::Vec3::Z, glam::Vec3::Y),
                    // back
                    5 => point_light.view_transform_in_direction(-glam::Vec3::Z, glam::Vec3::Y),
                    _ => unreachable!(),
                };

                light_views.push(view_transform);
            }

            queue.write_buffer(
                &self.point_light_view_transform_buffer,
                0,
                bytemuck::cast_slice(&light_views),
            );

            encoder.insert_debug_marker("Shadow Cube Map Render Pass Begin");
            // build the shadow cube map
            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Shadow Cube Map Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.shadow_cube_d2array_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.shadow_cube_depth_target_d2array_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.shadow_cube_map_pipeline);
                render_pass.set_bind_group(0, &self.shadow_cube_map_bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(
                    0..mesh.num_indices() as u32,
                    0,
                    0..transforms.len() as u32,
                );
            }

            encoder.insert_debug_marker("Shadow Cube Map Render Pass End");

            queue.submit(std::iter::once(encoder.finish()));
        }

        Ok(())
    }

    fn overlay_cube_shadow_map(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_target: &Texture,
        depth_target: &Texture,
        camera: &Camera,
        unique_meshes: &FxHashMap<AssetId, UniqueMesh>,
    ) -> anyhow::Result<()> {
        let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Shadow Cube Overlay Initial Encoder"),
        });

        let camera_uniform = CameraUniform::from(camera);

        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );

        queue.submit(std::iter::once(encoder.finish()));

        // overlay the built shadow cube map on the screen
        for unique_mesh in unique_meshes.values() {
            let UniqueMesh { mesh, transforms } = unique_mesh;
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Cube Overlay Buffer Write Encoder"),
            });

            queue.write_buffer(
                &self.model_transform_buffer,
                0,
                bytemuck::cast_slice(transforms.as_slice()),
            );

            encoder.insert_debug_marker("Shadow Cube Overlay Render Pass Begin");

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Shadow Cube Overlay Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color_target.view(),
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_target.view(),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.shadow_cube_overlay_pipeline);
                render_pass.set_bind_group(0, &self.shadow_cube_overlay_bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(
                    0..mesh.num_indices() as u32,
                    0,
                    0..transforms.len() as u32,
                );
            }

            encoder.insert_debug_marker("Shadow Cube Overlay Render Pass End");

            queue.submit(std::iter::once(encoder.finish()));
        }

        Ok(())
    }
}

impl Pass for ShadowRenderPass {
    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn enable(&mut self) {
        self.enabled = true;
    }

    fn disable(&mut self) {
        self.enabled = false;
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_target: &Texture,
        depth_target: &Texture,
        world: &World,
    ) -> anyhow::Result<()> {
        let camera = Query::<&Camera>::new(world);
        let camera = camera.iter().next().unwrap();

        let unique_meshes = UniqueMesh::gather(world);

        for light in Query::<&DirectionalLight>::new(world).iter() {
            self.render_shadow_map(device, queue, &light, &camera, &unique_meshes)?;
            self.overlay_shadow_map(
                device,
                queue,
                color_target,
                depth_target,
                &camera,
                &unique_meshes,
            )?;
        }

        for light in Query::<&PointLight>::new(world).iter() {
            self.render_cube_map(device, queue, &light, &unique_meshes)?;
            self.overlay_cube_shadow_map(
                device,
                queue,
                color_target,
                depth_target,
                &camera,
                &unique_meshes,
            )?;
        }

        Ok(())
    }
}
