use std::{num::NonZeroU32, sync::Arc};

use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use weaver_proc_macro::{GpuComponent, StaticId};

use crate::{
    core::{
        light::{PointLightArray, MAX_LIGHTS},
        mesh::Vertex,
        texture::{
            DepthCubeArrayTexture, DepthTexture, MonoCubeArrayTexture, MonoCubeTexture,
            MonoTexture, TextureFormat, WindowTexture,
        },
        transform::TransformArray,
    },
    include_shader,
    prelude::*,
    renderer::{
        internals::{
            BindGroupLayoutCache, BindableComponent, GpuComponent, GpuResourceManager,
            GpuResourceType, LazyBindGroup, LazyGpuHandle,
        },
        Renderer,
    },
};

use super::Pass;

const SHADOW_DEPTH_TEXTURE_SIZE: u32 = 1024;

#[derive(StaticId, GpuComponent)]
#[gpu(update = "update")]
struct UniqueMesh {
    mesh: Mesh,
    #[gpu(component)]
    transforms: TransformArray,
}

impl UniqueMesh {
    fn update(&self, _world: &World) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Default, StaticId, GpuComponent)]
#[gpu(update = "update")]
struct UniqueMeshes {
    #[gpu(component)]
    unique_meshes: FxHashMap<u64, UniqueMesh>,
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
                .entry(mesh.asset_id().id())
                .or_insert_with(|| UniqueMesh {
                    mesh: mesh.clone(),
                    transforms: TransformArray::new(),
                });

            unique_mesh.transforms.push(&transform);
        }
    }

    fn update(&self, _world: &World) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Clone, StaticId, GpuComponent, BindableComponent)]
#[gpu(update = "update")]
struct LightViews {
    #[storage]
    handle: LazyGpuHandle,
    bind_group: LazyBindGroup<Self>,
}

impl LightViews {
    pub fn new() -> Self {
        Self {
            handle: LazyGpuHandle::new(
                GpuResourceType::Storage {
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    size: std::mem::size_of::<glam::Mat4>() * 6,
                    read_only: true,
                },
                Some("Light Views"),
                None,
            ),
            bind_group: LazyBindGroup::default(),
        }
    }

    fn update(&self, _world: &World) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(StaticId, Clone, GpuComponent)]
#[gpu(update = "update")]
struct ShadowBuffers {
    shadow_cubemap: LazyGpuHandle,
    bind_group: LazyBindGroup<Self>,
}

impl ShadowBuffers {
    fn update(&self, _world: &World) -> anyhow::Result<()> {
        Ok(())
    }
}

impl BindableComponent for ShadowBuffers {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Buffers Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: MonoCubeArrayTexture::SAMPLE_TYPE,
                        view_dimension: wgpu::TextureViewDimension::CubeArray,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        })
    }

    fn create_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        let shadow_cubemap = self.shadow_cubemap.lazy_init(manager)?;
        let shadow_cubemap = shadow_cubemap.get_texture().unwrap();
        let shadow_cubemap_view = shadow_cubemap.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Shadow Cubemap View"),
            format: Some(MonoTexture::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::CubeArray),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: Some(6 * MAX_LIGHTS as u32),
            mip_level_count: None,
        });

        let sampler = manager.device().create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Cubemap Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = cache.get_or_create::<Self>(manager.device());
        let bind_group = manager
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Shadow Buffers Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&shadow_cubemap_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });

        Ok(Arc::new(bind_group))
    }

    fn bind_group(&self) -> Option<Arc<wgpu::BindGroup>> {
        self.bind_group.bind_group().clone()
    }

    fn lazy_init_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        if let Some(bind_group) = self.bind_group.bind_group() {
            return Ok(bind_group);
        }

        let bind_group = self.bind_group.lazy_init_bind_group(manager, cache, self)?;
        Ok(bind_group)
    }
}

pub struct OmniShadowRenderPass {
    enabled: bool,

    cubemap_pipeline: wgpu::RenderPipeline,
    shadow_buffers: RwLock<ShadowBuffers>,
    shadow_depth_cubemap: LazyGpuHandle,

    overlay_pipeline: wgpu::RenderPipeline,

    light_views: RwLock<Vec<LightViews>>,
    unique_meshes: RwLock<UniqueMeshes>,
}

impl OmniShadowRenderPass {
    pub fn new(device: &wgpu::Device, layout_cache: &BindGroupLayoutCache) -> Self {
        let cubemap_shader = device.create_shader_module(include_shader!("shadow_cubemap.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow_cubemap"),
            bind_group_layouts: &[
                // model transforms
                &layout_cache.get_or_create::<TransformArray>(device),
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
                entry_point: "shadow_cubemap_vs",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &cubemap_shader,
                entry_point: "shadow_cubemap_fs",
                targets: &[Some(wgpu::ColorTargetState {
                    format: MonoCubeTexture::FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: Some(NonZeroU32::new(6).unwrap()),
        });

        let shadow_cubemap = LazyGpuHandle::new(
            GpuResourceType::Texture {
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: MonoCubeTexture::FORMAT,
                width: SHADOW_DEPTH_TEXTURE_SIZE,
                height: SHADOW_DEPTH_TEXTURE_SIZE,
                dimension: wgpu::TextureDimension::D2,
                view_dimension: wgpu::TextureViewDimension::CubeArray,
                depth_or_array_layers: 6 * MAX_LIGHTS as u32,
            },
            Some("Shadow Cubemap"),
            None,
        );

        let shadow_depth_cubemap = LazyGpuHandle::new(
            GpuResourceType::Texture {
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: DepthCubeArrayTexture::FORMAT,
                width: SHADOW_DEPTH_TEXTURE_SIZE,
                height: SHADOW_DEPTH_TEXTURE_SIZE,
                dimension: wgpu::TextureDimension::D2,
                view_dimension: wgpu::TextureViewDimension::CubeArray,
                depth_or_array_layers: 6 * MAX_LIGHTS as u32,
            },
            Some("Shadow Depth Cubemap"),
            None,
        );

        let overlay_shader =
            device.create_shader_module(include_shader!("shadow_cubemap_overlay.wgsl"));

        let overlay_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow_cubemap_overlay"),
                bind_group_layouts: &[
                    // model transforms
                    &layout_cache.get_or_create::<TransformArray>(device),
                    // camera
                    &layout_cache.get_or_create::<Camera>(device),
                    // shadow cubemap and sampler
                    &layout_cache.get_or_create::<ShadowBuffers>(device),
                    // light
                    &layout_cache.get_or_create::<PointLightArray>(device),
                ],
                push_constant_ranges: &[],
            });

        let overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow_cubemap_overlay"),
            layout: Some(&overlay_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &overlay_shader,
                entry_point: "shadow_cubemap_overlay_vs",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &overlay_shader,
                entry_point: "shadow_cubemap_overlay_fs",
                targets: &[Some(wgpu::ColorTargetState {
                    format: WindowTexture::FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::FORMAT,
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
            shadow_buffers: RwLock::new(ShadowBuffers {
                shadow_cubemap,
                bind_group: LazyBindGroup::default(),
            }),
            shadow_depth_cubemap,

            overlay_pipeline,

            light_views: RwLock::new(Vec::new()),
            unique_meshes: RwLock::new(UniqueMeshes::default()),
        }
    }

    fn render_cube_map(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        renderer: &Renderer,
        point_light: &PointLight,
        point_light_index: usize,
    ) -> anyhow::Result<()> {
        let manager = &renderer.resource_manager;
        let cache = &renderer.bind_group_layout_cache;
        let point_light_bind_group = point_light.lazy_init_bind_group(manager, cache)?;

        let light_views = self.light_views.read();
        let light_views = light_views.get(point_light_index).unwrap();
        let light_views_bind_group = light_views.lazy_init_bind_group(manager, cache)?;

        let shadow_cubemap_handle = self
            .shadow_buffers
            .read()
            .shadow_cubemap
            .lazy_init(manager)?;
        let shadow_depth_cubemap_handle = self.shadow_depth_cubemap.lazy_init(manager)?;

        let shadow_cubemap = shadow_cubemap_handle.get_texture().unwrap();
        let shadow_depth_cubemap = shadow_depth_cubemap_handle.get_texture().unwrap();

        let shadow_cubemap_view = shadow_cubemap.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Shadow Cubemap View"),
            format: Some(MonoTexture::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: point_light_index as u32 * 6,
            array_layer_count: Some(6),
            mip_level_count: None,
        });

        let shadow_depth_cubemap_view =
            shadow_depth_cubemap.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Shadow Depth Cubemap View"),
                format: Some(DepthTexture::FORMAT),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                base_array_layer: point_light_index as u32 * 6,
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
        for mesh in self.unique_meshes.read().unique_meshes.values() {
            let UniqueMesh { mesh, transforms } = mesh;

            let transform_bind_group = transforms.lazy_init_bind_group(manager, cache)?;

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
    ) -> anyhow::Result<()> {
        let camera = Query::<&Camera>::new(world);
        let camera = camera.iter().next().unwrap();
        let camera_bind_group = camera.lazy_init_bind_group(
            &renderer.resource_manager,
            &renderer.bind_group_layout_cache,
        )?;

        let point_lights_bind_group = renderer.point_lights.lazy_init_bind_group(
            &renderer.resource_manager,
            &renderer.bind_group_layout_cache,
        )?;

        let shadow_buffers = self.shadow_buffers.read();
        let shadow_buffers_bind_group = shadow_buffers.lazy_init_bind_group(
            &renderer.resource_manager,
            &renderer.bind_group_layout_cache,
        )?;

        for mesh in self.unique_meshes.read().unique_meshes.values() {
            let UniqueMesh { mesh, transforms } = mesh;

            let transform_bind_group = transforms.lazy_init_bind_group(
                &renderer.resource_manager,
                &renderer.bind_group_layout_cache,
            )?;

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
                render_pass.set_bind_group(3, &point_lights_bind_group, &[]);
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
            .read()
            .shadow_cubemap
            .lazy_init(&renderer.resource_manager)?;
        self.shadow_depth_cubemap
            .lazy_init(&renderer.resource_manager)?;

        self.unique_meshes.write().gather(world);
        self.unique_meshes
            .read()
            .lazy_init(&renderer.resource_manager)?;
        self.unique_meshes.write().update_resources(world)?;

        let point_lights = Query::<&PointLight>::new(world);
        for (i, entity) in point_lights.entities().enumerate() {
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

            let mut light_views = self.light_views.write();
            let light_views = match light_views.get(i) {
                Some(light_views) => light_views,
                None => {
                    let lv = LightViews::new();
                    lv.lazy_init(&renderer.resource_manager)?;
                    light_views.push(lv);
                    light_views.last().unwrap()
                }
            };
            light_views.handle.update(&views);
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
        for (i, entity) in lights.entities().enumerate() {
            let light = lights.get(entity).unwrap();
            self.render_cube_map(encoder, renderer, &light, i)?;
        }
        self.overlay_shadow_cube_map(encoder, color_target, depth_target, renderer, world)?;

        Ok(())
    }
}
