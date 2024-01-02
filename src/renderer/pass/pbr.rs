use rustc_hash::FxHashMap;

use crate::{
    app::asset_server::AssetId,
    core::{
        camera::{CameraUniform, FlyCamera},
        light::{DirectionalLight, DirectionalLightBuffer, PointLight, PointLightBuffer},
        material::{Material, MaterialUniform},
        mesh::{Mesh, Vertex, MAX_MESHES},
        texture::Texture,
        transform::Transform,
    },
    ecs::{Query, Queryable, Read, World, Write},
    include_shader,
};

struct UniqueMesh {
    mesh: Mesh,
    material: Material,
    transforms: Vec<Transform>,
}

impl UniqueMesh {
    fn gather(world: &World) -> FxHashMap<(AssetId, AssetId), Self> {
        let query = world.query::<Query<(Read<Material>, Read<Mesh>, Read<Transform>)>>();
        // gather all entities that share a mesh
        let mut unique_meshes = FxHashMap::default();
        for entity in query.entities() {
            let (material, mesh, transform) = query.get(entity).unwrap();
            let mesh_id = mesh.asset_id();
            let material_id = material.asset_id();
            unique_meshes
                .entry((mesh_id, material_id))
                .or_insert_with(|| UniqueMesh {
                    mesh: mesh.clone(),
                    material: material.clone(),
                    transforms: Vec::new(),
                })
                .transforms
                .push(*transform);
        }
        unique_meshes
    }
}

pub struct PbrRenderPass {
    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) pipeline: wgpu::RenderPipeline,

    pub(crate) model_transform_buffer: wgpu::Buffer,
    pub(crate) camera_buffer: wgpu::Buffer,
    pub(crate) material_buffer: wgpu::Buffer,
    pub(crate) point_light_buffer: wgpu::Buffer,
    pub(crate) directional_light_buffer: wgpu::Buffer,
}

impl PbrRenderPass {
    pub fn new(device: &wgpu::Device, env_map_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let model_transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Model Transform Buffer"),
            size: (std::mem::size_of::<glam::Mat4>() * MAX_MESHES) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Material Buffer"),
            size: std::mem::size_of::<MaterialUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let point_light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Point Lights Buffer"),
            size: std::mem::size_of::<PointLightBuffer>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let directional_light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Directional Lights Buffer"),
            size: std::mem::size_of::<DirectionalLightBuffer>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PBR Bind Group Layout"),
            entries: &[
                // model_transform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // camera
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
                // material
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // tex_sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // tex
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // normal_tex
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // roughness_tex
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // ambient occlusion texture
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // point lights
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // directional lights
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PBR Shader"),
            source: wgpu::ShaderSource::Wgsl(include_shader!("pbr.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PBR Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout, &env_map_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PBR Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: Texture::HDR_FORMAT,
                    blend: None,
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
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            bind_group_layout,
            pipeline,
            model_transform_buffer,
            camera_buffer,
            material_buffer,
            point_light_buffer,
            directional_light_buffer,
        }
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_texture: &Texture,
        depth_texture: &Texture,
        env_map_bind_group: &wgpu::BindGroup,
        world: &World,
    ) -> anyhow::Result<()> {
        let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("PBR Render Pass Initial Encoder"),
        });

        // write buffers
        let camera = world.read_resource::<FlyCamera>();
        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[CameraUniform::from(*camera)]),
        );
        {
            // write point lights buffer
            let lights = world.query::<Query<Read<PointLight>>>();
            let lights_uniform = lights.iter().map(|l| *l).collect::<Vec<_>>();
            queue.write_buffer(
                &self.point_light_buffer,
                0,
                bytemuck::cast_slice(&[PointLightBuffer::from(lights_uniform.as_slice())]),
            );
        }
        {
            // write directional lights buffer
            let lights = world.query::<Query<Read<DirectionalLight>>>();
            let lights_uniform = lights.iter().map(|l| *l).collect::<Vec<_>>();
            queue.write_buffer(
                &self.directional_light_buffer,
                0,
                bytemuck::cast_slice(&[DirectionalLightBuffer::from(lights_uniform.as_slice())]),
            );
        }

        queue.submit(Some(encoder.finish()));

        let mut unique_meshes = UniqueMesh::gather(world);

        for unique_mesh in unique_meshes.values_mut() {
            let UniqueMesh {
                mesh,
                material,
                transforms,
            } = unique_mesh;

            let bind_group = material.bind_group().unwrap();

            let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("PBR Render Pass Buffer Write Encoder"),
            });

            // write model transform buffer
            queue.write_buffer(
                &self.model_transform_buffer,
                0,
                bytemuck::cast_slice(transforms.as_slice()),
            );

            // write material buffer
            queue.write_buffer(
                &self.material_buffer,
                0,
                bytemuck::cast_slice(&[MaterialUniform::from(&*material)]),
            );

            queue.submit(Some(encoder.finish()));

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("PBR Render Pass Encoder"),
            });

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("PBR Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color_texture.view(),
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_texture.view(),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                });

                render_pass.set_pipeline(&self.pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.set_bind_group(1, env_map_bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(
                    0..mesh.num_indices() as u32,
                    0,
                    0..transforms.len() as u32,
                );
            }

            queue.submit(Some(encoder.finish()));
        }

        Ok(())
    }

    pub fn prepare_components(&self, world: &World, renderer: &crate::Renderer) {
        let query = world.query::<Query<Write<Material>>>();
        for mut material in query.iter() {
            if !material.has_bind_group() {
                material.create_bind_group(
                    &renderer.device,
                    &renderer.pbr_pass,
                    &renderer.sampler_repeat_linear,
                );
            }
        }
    }
}
