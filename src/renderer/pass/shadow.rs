use std::{cell::RefCell, num::NonZeroU32, sync::Arc};

use rustc_hash::FxHashMap;

use crate::{
    app::asset_server::AssetId,
    core::{
        mesh::{Vertex, MAX_MESHES},
        texture::{
            DepthCubeFormat, DepthFormat, MonoCubeFormat, MonoFormat, TextureFormat, WindowFormat,
        },
    },
    include_shader,
    prelude::*,
    renderer::{
        AllocBuffers, BindGroupLayoutCache, BufferHandle, CreateBindGroupLayout, LazyBufferHandle,
        Renderer,
    },
};

use super::Pass;

const SHADOW_DEPTH_TEXTURE_SIZE: u32 = 1024;

struct UniqueMesh {
    mesh: Mesh,
    transforms: Vec<Transform>,
    transform_buffer: LazyBufferHandle,
}

#[derive(Default)]
struct UniqueMeshes {
    unique_meshes: FxHashMap<AssetId, UniqueMesh>,
}

impl UniqueMeshes {
    pub fn gather(&mut self, world: &World) {
        let query = Query::<(&Mesh, &Transform)>::new(world);

        // clear the transforms
        for unique_mesh in self.unique_meshes.values_mut() {
            unique_mesh.transforms.clear();
        }

        for (mesh, transform) in query.iter() {
            let unique_mesh = self
                .unique_meshes
                .entry(mesh.asset_id())
                .or_insert_with(|| UniqueMesh {
                    mesh: mesh.clone(),
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

#[derive(Component, Clone)]
struct LightViews {
    handle: LazyBufferHandle,
}

impl LightViews {
    pub fn new() -> Self {
        Self {
            handle: LazyBufferHandle::new(
                crate::renderer::BufferBindingType::Storage {
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    size: Some(std::mem::size_of::<glam::Mat4>() * 6),
                    read_only: true,
                },
                Some("Light Views"),
                None,
            ),
        }
    }

    pub fn update(&self, light_views: &[glam::Mat4; 6]) {
        self.handle.update(light_views);
    }
}

impl AllocBuffers for LightViews {
    fn alloc_buffers(&self, renderer: &Renderer) -> anyhow::Result<Vec<BufferHandle>> {
        Ok(vec![self.handle.get_or_create::<Self>(renderer)])
    }
}

impl CreateBindGroupLayout for LightViews {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Light Views"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }
}

#[derive(Component, Clone)]
struct ShadowBuffers {
    shadow_cubemap: LazyBufferHandle,
    bind_group: Option<Arc<wgpu::BindGroup>>,
}

impl ShadowBuffers {
    pub fn get_or_create_bind_group(&mut self, renderer: &Renderer) -> Arc<wgpu::BindGroup> {
        if let Some(bind_group) = &self.bind_group {
            return bind_group.clone();
        }

        let shadow_cubemap_handle = self
            .shadow_cubemap
            .get_or_create::<MonoCubeFormat>(renderer);
        let shadow_cubemap = shadow_cubemap_handle.get_texture().unwrap();

        let bind_group = Arc::new(
            renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Shadow Overlay Buffers"),
                    layout: &renderer
                        .bind_group_layout_cache
                        .get_or_create::<Self>(&renderer.device),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                &shadow_cubemap.create_view(&wgpu::TextureViewDescriptor {
                                    label: Some("Shadow Cubemap View"),
                                    format: Some(MonoCubeFormat::FORMAT),
                                    dimension: Some(wgpu::TextureViewDimension::Cube),
                                    aspect: wgpu::TextureAspect::All,
                                    base_mip_level: 0,
                                    base_array_layer: 0,
                                    array_layer_count: None,
                                    mip_level_count: None,
                                }),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(
                                &renderer.sampler_clamp_nearest,
                            ),
                        },
                    ],
                }),
        );

        self.bind_group = Some(bind_group.clone());

        bind_group
    }
}

impl AllocBuffers for ShadowBuffers {
    fn alloc_buffers(&self, renderer: &Renderer) -> anyhow::Result<Vec<BufferHandle>> {
        Ok(vec![self
            .shadow_cubemap
            .get_or_create::<MonoCubeFormat>(renderer)])
    }
}

impl CreateBindGroupLayout for ShadowBuffers {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Overlay Buffers"),
            entries: &[
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
                // sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        })
    }
}

pub struct OmniShadowRenderPass {
    enabled: bool,

    cubemap_pipeline: wgpu::RenderPipeline,
    shadow_buffers: RefCell<ShadowBuffers>,
    shadow_depth_cubemap: LazyBufferHandle,

    overlay_pipeline: wgpu::RenderPipeline,

    light_views: RefCell<FxHashMap<Entity, LightViews>>,
    unique_meshes: RefCell<UniqueMeshes>,
}

impl OmniShadowRenderPass {
    pub fn new(device: &wgpu::Device, layout_cache: &BindGroupLayoutCache) -> Self {
        let cubemap_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow_cubemap"),
            source: wgpu::ShaderSource::Wgsl(include_shader!("shadow_cubemap.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow_cubemap"),
            bind_group_layouts: &[
                // model transforms
                &layout_cache.get_or_create::<Transform>(device),
                // point light
                &layout_cache.get_or_create::<PointLight>(device),
                // light views
                &layout_cache.get_or_create::<LightViews>(device),
            ],
            push_constant_ranges: &[],
        });

        let cubemap_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow_cubemap"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &cubemap_shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &cubemap_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: MonoCubeFormat::FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthFormat::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: Some(NonZeroU32::new(6).unwrap()),
        });

        let shadow_cubemap = LazyBufferHandle::new(
            crate::renderer::BufferBindingType::Texture {
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: MonoCubeFormat::FORMAT,
                width: SHADOW_DEPTH_TEXTURE_SIZE,
                height: SHADOW_DEPTH_TEXTURE_SIZE,
                dimension: wgpu::TextureDimension::D2,
                view_dimension: wgpu::TextureViewDimension::Cube,
                depth_or_array_layers: 6,
            },
            Some("Shadow Cubemap"),
            None,
        );

        let shadow_depth_cubemap = LazyBufferHandle::new(
            crate::renderer::BufferBindingType::Texture {
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: DepthCubeFormat::FORMAT,
                width: SHADOW_DEPTH_TEXTURE_SIZE,
                height: SHADOW_DEPTH_TEXTURE_SIZE,
                dimension: wgpu::TextureDimension::D2,
                view_dimension: wgpu::TextureViewDimension::Cube,
                depth_or_array_layers: 6,
            },
            Some("Shadow Depth Cubemap"),
            None,
        );

        let overlay_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow_cubemap_overlay"),
            source: wgpu::ShaderSource::Wgsl(include_shader!("shadow_cubemap_overlay.wgsl").into()),
        });

        let overlay_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow_cubemap_overlay"),
                bind_group_layouts: &[
                    // model transforms
                    &layout_cache.get_or_create::<Transform>(device),
                    // camera
                    &layout_cache.get_or_create::<Camera>(device),
                    // shadow cubemap and sampler
                    &layout_cache.get_or_create::<ShadowBuffers>(device),
                    // light
                    &layout_cache.get_or_create::<PointLight>(device),
                ],
                push_constant_ranges: &[],
            });

        let overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow_cubemap_overlay"),
            layout: Some(&overlay_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &overlay_shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &overlay_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: WindowFormat::FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthFormat::FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
        });

        Self {
            enabled: true,

            cubemap_pipeline,
            shadow_buffers: RefCell::new(ShadowBuffers {
                shadow_cubemap,
                bind_group: None,
            }),
            shadow_depth_cubemap,

            overlay_pipeline,

            light_views: RefCell::new(FxHashMap::default()),
            unique_meshes: RefCell::new(UniqueMeshes::default()),
        }
    }

    fn render_cube_map(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        renderer: &Renderer,
        point_light: &PointLight,
        point_light_entity: Entity,
    ) -> anyhow::Result<()> {
        let point_light_handle = &point_light.alloc_buffers(renderer).unwrap()[0];
        let point_light_bind_group = point_light_handle.bind_group().unwrap();

        let light_views = self.light_views.borrow();
        let light_views = light_views.get(&point_light_entity).unwrap();
        let light_views_handle = &light_views.alloc_buffers(renderer).unwrap()[0];
        let light_views_bind_group = light_views_handle.bind_group().unwrap();

        let shadow_cubemap_handle = self
            .shadow_buffers
            .borrow()
            .shadow_cubemap
            .get_or_create::<MonoCubeFormat>(renderer);
        let shadow_depth_cubemap_handle = self
            .shadow_depth_cubemap
            .get_or_create::<DepthCubeFormat>(renderer);

        let shadow_cubemap = shadow_cubemap_handle.get_texture().unwrap();
        let shadow_depth_cubemap = shadow_depth_cubemap_handle.get_texture().unwrap();

        let shadow_cubemap_view = shadow_cubemap.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Shadow Cubemap View"),
            format: Some(MonoFormat::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: Some(6),
            mip_level_count: None,
        });

        let shadow_depth_cubemap_view =
            shadow_depth_cubemap.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Shadow Depth Cubemap View"),
                format: Some(DepthFormat::FORMAT),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                base_array_layer: 0,
                array_layer_count: Some(6),
                mip_level_count: None,
            });

        // clear the shadow cubemap
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Shadow Cubemap"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &shadow_cubemap_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &shadow_depth_cubemap_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // render our meshes
        for mesh in self.unique_meshes.borrow().unique_meshes.values() {
            let UniqueMesh {
                mesh,
                transforms,
                transform_buffer,
            } = mesh;

            let transform_buffer = transform_buffer.get_or_create::<Transform>(renderer);
            let transform_bind_group = transform_buffer.bind_group().unwrap();

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Shadow Cubemap"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &shadow_cubemap_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &shadow_depth_cubemap_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                render_pass.set_pipeline(&self.cubemap_pipeline);
                render_pass.set_bind_group(0, &transform_bind_group, &[]);
                render_pass.set_bind_group(1, &point_light_bind_group, &[]);
                render_pass.set_bind_group(2, &light_views_bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(
                    0..mesh.num_indices() as u32,
                    0,
                    0..transforms.len() as u32,
                );
            }
        }

        Ok(())
    }

    fn overlay_shadow_cube_map(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_target: &wgpu::TextureView,
        depth_target: &wgpu::TextureView,
        renderer: &Renderer,
        world: &World,
        point_light: &PointLight,
    ) -> anyhow::Result<()> {
        let camera = Query::<&Camera>::new(world);
        let camera = camera.iter().next().unwrap();
        let camera_handle = &camera.alloc_buffers(renderer).unwrap()[0];
        let camera_bind_group = camera_handle.bind_group().unwrap();

        let point_light_handle = &point_light.alloc_buffers(renderer).unwrap()[0];
        let point_light_bind_group = point_light_handle.bind_group().unwrap();

        let mut shadow_buffers = self.shadow_buffers.borrow_mut();
        let shadow_buffers_bind_group = shadow_buffers.get_or_create_bind_group(renderer);

        for mesh in self.unique_meshes.borrow().unique_meshes.values() {
            let UniqueMesh {
                mesh,
                transforms,
                transform_buffer,
            } = mesh;

            let transform_buffer = transform_buffer.get_or_create::<Transform>(renderer);
            let transform_bind_group = transform_buffer.bind_group().unwrap();

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Shadow Cubemap Overlay"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color_target,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_target,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                render_pass.set_pipeline(&self.overlay_pipeline);
                render_pass.set_bind_group(0, &transform_bind_group, &[]);
                render_pass.set_bind_group(1, &camera_bind_group, &[]);
                render_pass.set_bind_group(2, &shadow_buffers_bind_group, &[]);
                render_pass.set_bind_group(3, &point_light_bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(
                    0..mesh.num_indices() as u32,
                    0,
                    0..transforms.len() as u32,
                );
            }
        }

        Ok(())
    }
}

impl Pass for OmniShadowRenderPass {
    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn enable(&mut self) {
        self.enabled = true;
    }

    fn disable(&mut self) {
        self.enabled = false;
    }

    fn prepare(&self, world: &World, renderer: &Renderer) -> anyhow::Result<()> {
        self.shadow_buffers
            .borrow()
            .shadow_cubemap
            .get_or_create::<MonoCubeFormat>(renderer);
        self.shadow_depth_cubemap
            .get_or_create::<DepthCubeFormat>(renderer);

        self.unique_meshes.borrow_mut().gather(world);
        self.unique_meshes.borrow().alloc_buffers(renderer)?;
        self.unique_meshes.borrow_mut().update();

        let point_lights = Query::<&PointLight>::new(world);
        for entity in point_lights.entities() {
            let point_light = point_lights.get(entity).unwrap();
            let mut views = [glam::Mat4::IDENTITY; 6];
            for (i, view) in views.iter_mut().enumerate() {
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

                *view = view_transform;
            }

            let mut light_views = self.light_views.borrow_mut();
            let light_views = light_views.entry(entity).or_insert_with(|| {
                let light_views = LightViews::new();
                light_views.alloc_buffers(renderer).unwrap();
                light_views
            });

            light_views.update(&views);
        }

        Ok(())
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_target: &wgpu::TextureView,
        depth_target: &wgpu::TextureView,
        renderer: &Renderer,
        world: &World,
    ) -> anyhow::Result<()> {
        let lights = Query::<&PointLight>::new(world);
        for entity in lights.entities() {
            let light = lights.get(entity).unwrap();
            self.render_cube_map(encoder, renderer, &light, entity)?;
            self.overlay_shadow_cube_map(
                encoder,
                color_target,
                depth_target,
                renderer,
                world,
                &light,
            )?;
        }

        Ok(())
    }
}
