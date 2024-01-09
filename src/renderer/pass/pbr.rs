use std::{cell::RefCell, sync::Arc};

use rustc_hash::FxHashMap;
use weaver_proc_macro::Component;

use crate::{
    app::asset_server::AssetId,
    core::{
        camera::{Camera, CameraUniform},
        light::PointLightArray,
        material::Material,
        mesh::{Mesh, Vertex, MAX_MESHES},
        texture::{DepthFormat, HdrCubeFormat, HdrFormat, NormalMapFormat, Skybox, TextureFormat},
        transform::Transform,
    },
    ecs::{Query, World},
    include_shader,
    renderer::{
        AllocBuffers, BindGroupLayoutCache, BufferHandle, BufferStorage, CreateBindGroupLayout,
        LazyBufferHandle, Renderer, UpdateStatus,
    },
};

use super::sky::SKYBOX_CUBEMAP_SIZE;

pub struct UniqueMesh {
    pub mesh: Mesh,
    pub material_bind_group: Arc<wgpu::BindGroup>,
    pub transforms: Vec<Transform>,
    pub transform_buffer: LazyBufferHandle,
}

#[derive(Default)]
pub struct UniqueMeshes {
    pub unique_meshes: FxHashMap<(AssetId, AssetId), UniqueMesh>,
}

impl UniqueMeshes {
    pub fn gather(&mut self, world: &World, renderer: &Renderer) {
        let query = Query::<(&Mesh, &mut Material, &Transform)>::new(world);

        // clear the transforms
        for unique_mesh in self.unique_meshes.values_mut() {
            unique_mesh.transforms.clear();
        }

        for (mesh, mut material, transform) in query.iter() {
            let unique_mesh = self
                .unique_meshes
                .entry((mesh.asset_id(), material.asset_id()))
                .or_insert_with(|| UniqueMesh {
                    mesh: mesh.clone(),
                    material_bind_group: material.create_bind_group(renderer).unwrap(),
                    transforms: vec![],
                    transform_buffer: LazyBufferHandle::new(
                        crate::renderer::BufferBindingType::Storage {
                            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                            size: Some(std::mem::size_of::<glam::Mat4>() * MAX_MESHES),
                            read_only: true,
                        },
                        Some("Unique Mesh Transforms"),
                        None,
                    ),
                });

            unique_mesh.transforms.push(*transform);
        }
    }

    pub fn update(&mut self) {
        for unique_mesh in self.unique_meshes.values_mut() {
            unique_mesh.transform_buffer.update(&unique_mesh.transforms);
        }
    }
}

impl AllocBuffers for UniqueMeshes {
    fn alloc_buffers(&self, renderer: &Renderer) -> anyhow::Result<Vec<BufferHandle>> {
        let mut handles = Vec::new();
        for unique_mesh in self.unique_meshes.values() {
            handles.push(
                unique_mesh
                    .transform_buffer
                    .get_or_create::<Transform>(renderer),
            );
        }
        Ok(handles)
    }
}

#[derive(Clone, Component)]
pub struct PbrBuffers {
    pub(crate) camera: LazyBufferHandle,
    pub(crate) env_map: LazyBufferHandle,
    pub(crate) bind_group: RefCell<Option<Arc<wgpu::BindGroup>>>,
}

impl PbrBuffers {
    pub fn new() -> Self {
        Self {
            camera: LazyBufferHandle::new(
                crate::renderer::BufferBindingType::Uniform {
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    size: Some(std::mem::size_of::<CameraUniform>()),
                },
                Some("PBR Camera"),
                None,
            ),
            env_map: LazyBufferHandle::new(
                crate::renderer::BufferBindingType::Texture {
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    format: HdrCubeFormat::FORMAT,
                    width: SKYBOX_CUBEMAP_SIZE,
                    height: SKYBOX_CUBEMAP_SIZE,
                    dimension: wgpu::TextureDimension::D2,
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    depth_or_array_layers: 6,
                },
                Some("PBR Environment Map"),
                None,
            ),
            bind_group: RefCell::new(None),
        }
    }

    pub fn get_or_create_bind_group(&self, renderer: &Renderer) -> Arc<wgpu::BindGroup> {
        let mut bind_group = self.bind_group.borrow_mut();
        if bind_group.is_none() {
            let camera = self.camera.get_or_create::<Camera>(renderer);
            let env_map = self.env_map.get_or_create::<HdrCubeFormat>(renderer);
            let status = &*env_map.status.borrow();
            let view = match status {
                UpdateStatus::Ready { buffer } => match &*buffer.storage {
                    BufferStorage::Texture { view, .. } => view,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            };

            let env_map_sampler = renderer.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("PBR Env Map Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });

            *bind_group = Some(Arc::new(
                renderer
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("PBR Bind Group"),
                        layout: &renderer
                            .bind_group_layout_cache
                            .get_or_create::<PbrBuffers>(&renderer.device),
                        entries: &[
                            // camera
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: camera.get_buffer().unwrap().as_entire_binding(),
                            },
                            // env map
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(view),
                            },
                            // env map sampler
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::Sampler(&env_map_sampler),
                            },
                        ],
                    }),
            ));
        }

        bind_group.as_ref().unwrap().clone()
    }
}

impl Default for PbrBuffers {
    fn default() -> Self {
        Self::new()
    }
}

impl AllocBuffers for PbrBuffers {
    fn alloc_buffers(&self, renderer: &Renderer) -> anyhow::Result<Vec<BufferHandle>> {
        Ok(vec![
            self.camera.get_or_create::<Camera>(renderer),
            self.env_map.get_or_create::<HdrCubeFormat>(renderer),
        ])
    }
}

impl CreateBindGroupLayout for PbrBuffers {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PBR Buffers"),
            entries: &[
                // camera
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // env map
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                // env map sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        })
    }
}

pub struct PbrRenderPass {
    pipeline: wgpu::RenderPipeline,
    buffers: PbrBuffers,
    unique_meshes: RefCell<UniqueMeshes>,
}

impl PbrRenderPass {
    pub fn new(device: &wgpu::Device, bind_group_layout_cache: &BindGroupLayoutCache) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PBR Shader"),
            source: wgpu::ShaderSource::Wgsl(include_shader!("pbr.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PBR Pipeline Layout"),
            bind_group_layouts: &[
                // mesh transform
                &bind_group_layout_cache.get_or_create::<Transform>(device),
                // camera and env map
                &bind_group_layout_cache.get_or_create::<PbrBuffers>(device),
                // material
                &bind_group_layout_cache.get_or_create::<Material>(device),
                // point lights
                &bind_group_layout_cache.get_or_create::<PointLightArray>(device),
            ],
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
                targets: &[
                    // color target
                    Some(wgpu::ColorTargetState {
                        format: HdrFormat::FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthFormat::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let unique_meshes = RefCell::new(UniqueMeshes::default());

        Self {
            pipeline,
            buffers: PbrBuffers::new(),
            unique_meshes,
        }
    }

    pub fn prepare(&self, world: &World, renderer: &Renderer) {
        let mut unique_meshes = self.unique_meshes.borrow_mut();
        unique_meshes.gather(world, renderer);
        unique_meshes.alloc_buffers(renderer).unwrap();
        self.buffers.alloc_buffers(renderer).unwrap();
        unique_meshes.update();
    }

    pub fn render(
        &self,
        renderer: &Renderer,
        hdr_pass_view: &wgpu::TextureView,
        world: &World,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        let skybox = Query::<&Skybox>::new(world);
        let skybox = skybox.iter().next().unwrap();

        let skybox_handle = &skybox.texture.alloc_buffers(renderer)?[0];
        let skybox_texture = skybox_handle.get_texture().unwrap();

        let camera = Query::<&Camera>::new(world);
        let camera = camera.iter().next().unwrap();

        let camera_handle = &camera.alloc_buffers(renderer)?[0];

        let my_handles = self.buffers.alloc_buffers(renderer)?;
        let my_camera_buffer = my_handles[0].get_buffer().unwrap();
        let my_env_map_texture = my_handles[1].get_texture().unwrap();

        encoder.copy_buffer_to_buffer(
            &camera_handle.get_buffer().unwrap(),
            0,
            &my_camera_buffer,
            0,
            std::mem::size_of::<CameraUniform>() as u64,
        );

        encoder.copy_texture_to_texture(
            skybox_texture.as_image_copy(),
            my_env_map_texture.as_image_copy(),
            skybox_texture.size(),
        );

        let buffer_bind_group = self.buffers.get_or_create_bind_group(renderer);

        let point_lights_handle = &renderer.point_lights.alloc_buffers(renderer)?[0];
        let point_lights_bind_group = point_lights_handle.bind_group().unwrap();

        for unique_mesh in self.unique_meshes.borrow().unique_meshes.values() {
            let UniqueMesh {
                mesh,
                material_bind_group,
                transform_buffer,
                transforms,
            } = unique_mesh;

            // make sure the buffers are allocated
            let transform_handle = transform_buffer.get_or_create::<Transform>(renderer);
            let transform_bind_group = transform_handle.bind_group().unwrap();

            // don't need the mesh handle since it only has vertex and index buffers

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PBR Render Pass"),
                color_attachments: &[
                    // color target
                    Some(wgpu::RenderPassColorAttachment {
                        view: hdr_pass_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &renderer.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &transform_bind_group, &[]);
            render_pass.set_bind_group(1, &buffer_bind_group, &[]);
            render_pass.set_bind_group(2, material_bind_group, &[]);
            render_pass.set_bind_group(3, &point_lights_bind_group, &[]);
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
            render_pass.set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.num_indices() as u32, 0, 0..transforms.len() as u32);
        }

        Ok(())
    }
}
