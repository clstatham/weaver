use std::{borrow::BorrowMut, cell::RefCell, sync::Arc};

use rustc_hash::FxHashMap;

use crate::{
    app::asset_server::AssetId,
    core::{
        camera::Camera,
        light::PointLightArray,
        material::Material,
        mesh::{Mesh, Vertex, MAX_MESHES},
        texture::{DepthFormat, HdrFormat, NormalMapFormat, TextureFormat, WindowFormat},
        transform::Transform,
    },
    ecs::{Query, World},
    include_shader,
    renderer::{AllocBuffers, BindGroupLayoutCache, BufferHandle, LazyBufferHandle, Renderer},
};

pub(crate) struct UniqueMesh {
    mesh: Mesh,
    material_bind_group: Arc<wgpu::BindGroup>,
    transforms: Vec<Transform>,
    transform_buffer: LazyBufferHandle,
}

#[derive(Default)]
pub(crate) struct UniqueMeshes {
    pub(crate) unique_meshes: FxHashMap<(AssetId, AssetId), UniqueMesh>,
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

pub struct PbrRenderPass {
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) unique_meshes: RefCell<UniqueMeshes>,
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
                // camera
                &bind_group_layout_cache.get_or_create::<Camera>(device),
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
                    Some(wgpu::ColorTargetState {
                        format: HdrFormat::FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        format: NormalMapFormat::FORMAT,
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
            unique_meshes,
        }
    }

    pub fn prepare(&self, world: &World, renderer: &Renderer) {
        let mut unique_meshes = self.unique_meshes.borrow_mut();
        unique_meshes.gather(world, renderer);
        unique_meshes.alloc_buffers(renderer).unwrap();
        unique_meshes.update();
    }

    pub fn render(
        &self,
        renderer: &Renderer,
        hdr_pass_view: &wgpu::TextureView,
        world: &World,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        let camera = Query::<&Camera>::new(world);
        let camera = camera.iter().next().unwrap();

        let camera_handle = &camera.alloc_buffers(renderer)?[0];
        let camera_bind_group = camera_handle.bind_group().unwrap();

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
                    // normal target
                    Some(wgpu::RenderPassColorAttachment {
                        view: &renderer.normal_texture_view,
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
            render_pass.set_bind_group(1, &camera_bind_group, &[]);
            render_pass.set_bind_group(2, material_bind_group, &[]);
            render_pass.set_bind_group(3, &point_lights_bind_group, &[]);
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
            render_pass.set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.num_indices() as u32, 0, 0..transforms.len() as u32);
        }

        Ok(())
    }
}
