use std::sync::Arc;

use weaver_ecs::prelude::*;
use weaver_proc_macro::{BindableComponent, GpuComponent};

use crate::{
    camera::{Camera, CameraUniform},
    geom::Ray,
    input::Input,
    load_shader,
    mesh::{Mesh, Vertex},
    prelude::{GlobalTransform, Material, Renderer, Texture},
    renderer::internals::*,
    transform::TransformGpuComponent,
};

#[derive(Component, GpuComponent, BindableComponent)]
#[gpu(update = "update")]
pub struct EntityGpuBuffer {
    pub entity: Entity,

    #[uniform]
    #[gpu(handle)]
    pub(crate) entity_buffer: LazyGpuHandle,
    pub(crate) bind_group: LazyBindGroup<Self>,
}

impl EntityGpuBuffer {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            entity_buffer: LazyGpuHandle::new(
                GpuResourceType::Uniform {
                    usage: wgpu::BufferUsages::COPY_SRC
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::UNIFORM,
                    size: std::mem::size_of::<Entity>(),
                },
                Some("Entity Gpu Buffer"),
                None,
            ),
            bind_group: LazyBindGroup::default(),
        }
    }

    pub fn update(&self, _world: &World) -> anyhow::Result<()> {
        self.entity_buffer.update(&[self.entity]);

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickResult {
    pub entity: Entity,
    pub position: glam::Vec3,
    pub normal: glam::Vec3,
    pub ray: Ray,
}

// broad phase ray intersection test with mesh AABBs
pub(crate) struct BroadPhaseRayIntersection {
    mesh: Mesh,
    material_bind_group: Arc<wgpu::BindGroup>,
}

#[derive(Component, GpuComponent, BindableComponent)]
#[gpu(update = "update")]
pub struct ScreenPicker {
    pub(crate) screen_width: u32,
    pub(crate) screen_height: u32,
    pub(crate) screen_pos: Arc<RwLock<(u32, u32)>>,

    pub(crate) ray: Arc<RwLock<Option<Ray>>>,

    pub(crate) render_pipeline: wgpu::RenderPipeline,
    pub(crate) bind_group: LazyBindGroup<Self>,
    pub(crate) transforms: Arc<RwLock<Vec<TransformGpuComponent>>>,
    pub(crate) entities: Arc<RwLock<Vec<EntityGpuBuffer>>>,
    pub(crate) intersections: Arc<RwLock<Vec<BroadPhaseRayIntersection>>>,
    #[gpu(handle)]
    #[uniform]
    pub(crate) camera_uniform: LazyGpuHandle,
    #[gpu(handle)]
    #[uniform]
    pub(crate) entity_uniform: LazyGpuHandle,

    #[gpu(component)]
    pub(crate) position_texture: Texture,
    #[gpu(component)]
    pub(crate) normal_texture: Texture,
    #[gpu(component)]
    pub(crate) entity_texture: Texture,

    #[gpu(handle)]
    pub(crate) position_buffer: LazyGpuHandle,
    #[gpu(handle)]
    pub(crate) normal_buffer: LazyGpuHandle,
    #[gpu(handle)]
    pub(crate) entity_buffer: LazyGpuHandle,
}

impl ScreenPicker {
    pub fn new(
        device: &wgpu::Device,
        bind_group_layout_cache: &BindGroupLayoutCache,
        screen_width: u32,
        screen_height: u32,
    ) -> Self {
        let camera_uniform = LazyGpuHandle::new(
            GpuResourceType::Uniform {
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                size: std::mem::size_of::<CameraUniform>(),
            },
            Some("ScreenPicker Camera"),
            None,
        );

        let entity_uniform = LazyGpuHandle::new(
            GpuResourceType::Uniform {
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                size: std::mem::size_of::<Entity>(),
            },
            Some("ScreenPicker Entity Uniform"),
            None,
        );

        let mut transforms = Vec::new();
        let mut entities = Vec::new();
        for _ in 0..64 {
            let transform = TransformGpuComponent::new(glam::Mat4::IDENTITY);
            transforms.push(transform);
            let entity = EntityGpuBuffer::new(Entity::from_u64(0));
            entities.push(entity);
        }

        let position_texture = Texture::new_lazy(
            screen_width,
            screen_height,
            Some("ScreenPicker Entity Texture"),
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            wgpu::TextureFormat::Rgba32Float,
            wgpu::TextureDimension::D2,
            wgpu::TextureViewDimension::D2,
            1,
        );

        let normal_texture = Texture::new_lazy(
            screen_width,
            screen_height,
            Some("ScreenPicker Entity Texture"),
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            wgpu::TextureFormat::Rgba32Float,
            wgpu::TextureDimension::D2,
            wgpu::TextureViewDimension::D2,
            1,
        );

        let entity_texture = Texture::new_lazy(
            screen_width,
            screen_height,
            Some("ScreenPicker Entity Texture"),
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            wgpu::TextureFormat::Rg32Uint,
            wgpu::TextureDimension::D2,
            wgpu::TextureViewDimension::D2,
            1,
        );

        let render_shader = device.create_shader_module(load_shader!("picking_render.wgsl"));

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("ScreenPicker Render Pipeline Layout"),
                bind_group_layouts: &[
                    &bind_group_layout_cache.get_or_create::<Self>(device),
                    &bind_group_layout_cache.get_or_create::<TransformGpuComponent>(device),
                    &bind_group_layout_cache.get_or_create::<Material>(device),
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ScreenPicker Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: "picking_vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
                entry_point: "picking_fs_main",
                targets: &[
                    // position
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba32Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    // normal
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba32Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    // entity
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rg32Uint,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let position_buffer = LazyGpuHandle::new(
            GpuResourceType::Uniform {
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                size: std::mem::size_of::<glam::Vec4>()
                    * screen_width as usize
                    * screen_height as usize,
            },
            Some("ScreenPicker Position Buffer"),
            None,
        );

        let normal_buffer = LazyGpuHandle::new(
            GpuResourceType::Uniform {
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                size: std::mem::size_of::<glam::Vec4>()
                    * screen_width as usize
                    * screen_height as usize,
            },
            Some("ScreenPicker Normal Buffer"),
            None,
        );

        let entity_buffer = LazyGpuHandle::new(
            GpuResourceType::Uniform {
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                size: std::mem::size_of::<[u32; 2]>()
                    * screen_width as usize
                    * screen_height as usize,
            },
            Some("ScreenPicker Entity Buffer"),
            None,
        );

        let bind_group = LazyBindGroup::default();

        Self {
            screen_width,
            screen_height,
            ray: Arc::new(RwLock::new(None)),
            screen_pos: Arc::new(RwLock::new((0, 0))),
            transforms: Arc::new(RwLock::new(transforms)),
            entities: Arc::new(RwLock::new(entities)),
            intersections: Arc::new(RwLock::new(Vec::new())),
            render_pipeline,
            bind_group,
            camera_uniform,
            entity_uniform,
            position_texture,
            normal_texture,
            entity_texture,
            position_buffer,
            normal_buffer,
            entity_buffer,
        }
    }

    pub fn update(&self, _world: &World) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn prepare(&self, world: &World, renderer: &Renderer) -> anyhow::Result<()> {
        let input = world.read_resource::<Input>()?;
        let mouse_pos = input.mouse_position();
        let mut ray = None;
        if let Some(mouse_pos) = mouse_pos {
            if mouse_pos.x < 0.0 || mouse_pos.x >= self.screen_width as f32 {
                return Ok(());
            }
            if mouse_pos.y < 0.0 || mouse_pos.y >= self.screen_height as f32 {
                return Ok(());
            }
            *self.screen_pos.write() = (mouse_pos.x as u32, mouse_pos.y as u32);
            let camera = world.query::<&Camera>();
            let camera = camera.iter().next();
            if camera.is_none() {
                return Ok(());
            }
            let camera = camera.unwrap();

            ray = Some(camera.screen_to_ray(
                glam::Vec2::new(mouse_pos.x, mouse_pos.y),
                glam::Vec2::new(self.screen_width as f32, self.screen_height as f32),
            ));
        }

        self.camera_uniform.lazy_init(renderer.resource_manager())?;
        self.entity_uniform.lazy_init(renderer.resource_manager())?;
        self.position_texture
            .lazy_init(renderer.resource_manager())?;
        self.normal_texture.lazy_init(renderer.resource_manager())?;
        self.entity_texture.lazy_init(renderer.resource_manager())?;

        self.position_buffer
            .lazy_init(renderer.resource_manager())?;
        self.normal_buffer.lazy_init(renderer.resource_manager())?;
        self.entity_buffer.lazy_init(renderer.resource_manager())?;

        self.intersections.write().clear();

        if ray.is_none() {
            return Ok(());
        }
        let ray = ray.unwrap();

        let query = world.query::<(Entity, &Mesh, &GlobalTransform, &Material)>();
        for (i, (entity, mesh, transform, material)) in query.iter().enumerate() {
            let bounding = mesh.bounding_sphere();
            let bounding = bounding.transformed(*transform);
            let t = bounding.intersect_ray(ray.origin, ray.direction);

            if t.is_some() {
                let material_bind_group = material.lazy_init_bind_group(
                    renderer.resource_manager(),
                    &renderer.bind_group_layout_cache,
                )?;
                self.transforms.write()[i].matrix = transform.matrix;
                self.entities.write()[i].entity = entity;
                self.intersections.write().push(BroadPhaseRayIntersection {
                    mesh: mesh.clone(),
                    material_bind_group,
                });
            }
        }

        for transform in self.transforms.read().iter() {
            transform.lazy_init(renderer.resource_manager())?;
            transform.update_resources(world)?;
        }

        for entity in self.entities.read().iter() {
            entity.lazy_init(renderer.resource_manager())?;
            entity.update_resources(world)?;
        }

        self.ray.write().replace(ray);

        Ok(())
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        depth_target: &wgpu::TextureView,
        renderer: &Renderer,
        world: &World,
    ) -> anyhow::Result<()> {
        let camera = world.query::<&Camera>();
        let camera = camera.iter().next();
        if camera.is_none() {
            return Ok(());
        }
        let camera = camera.unwrap();

        let camera_handle = camera.handle.lazy_init(renderer.resource_manager())?;
        let camera_handle = camera_handle.get_buffer().unwrap();

        let my_camera_buffer = self.camera_uniform.lazy_init(renderer.resource_manager())?;
        let my_camera_buffer = my_camera_buffer.get_buffer().unwrap();

        let entity_buffer = self.entity_uniform.lazy_init(renderer.resource_manager())?;
        let entity_buffer = entity_buffer.get_buffer().unwrap();

        encoder.copy_buffer_to_buffer(
            &camera_handle,
            0,
            &my_camera_buffer,
            0,
            std::mem::size_of::<CameraUniform>() as u64,
        );

        let bind_group = self.lazy_init_bind_group(
            renderer.resource_manager(),
            &renderer.bind_group_layout_cache,
        )?;

        let position_texture = self
            .position_texture
            .handle()
            .lazy_init(renderer.resource_manager())?;
        let position_texture = position_texture.get_texture().unwrap();

        let normal_texture = self
            .normal_texture
            .handle()
            .lazy_init(renderer.resource_manager())?;
        let normal_texture = normal_texture.get_texture().unwrap();

        let entity_texture = self
            .entity_texture
            .handle()
            .lazy_init(renderer.resource_manager())?;
        let entity_texture = entity_texture.get_texture().unwrap();

        let position_view = position_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("ScreenPicker Position Texture View"),
            format: Some(wgpu::TextureFormat::Rgba32Float),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        let normal_view = normal_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("ScreenPicker Normal Texture View"),
            format: Some(wgpu::TextureFormat::Rgba32Float),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        let entity_view = entity_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("ScreenPicker Entity Texture View"),
            format: Some(wgpu::TextureFormat::Rg32Uint),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        {
            // clear textures
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ScreenPicker Render Pass"),
                color_attachments: &[
                    // position
                    Some(wgpu::RenderPassColorAttachment {
                        view: &position_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    // normal
                    Some(wgpu::RenderPassColorAttachment {
                        view: &normal_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    // entity
                    Some(wgpu::RenderPassColorAttachment {
                        view: &entity_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        for (i, broad_phase_ray_intersection) in self.intersections.write().drain(..).enumerate() {
            let BroadPhaseRayIntersection {
                mesh,
                material_bind_group,
            } = broad_phase_ray_intersection;

            let transform_bind_group = self.transforms.read()[i].lazy_init_bind_group(
                renderer.resource_manager(),
                &renderer.bind_group_layout_cache,
            )?;

            let this_entity_buffer = self.entities.read()[i]
                .entity_buffer
                .lazy_init(renderer.resource_manager())?;
            let this_entity_buffer = this_entity_buffer.get_buffer().unwrap();

            encoder.copy_buffer_to_buffer(
                &this_entity_buffer,
                0,
                &entity_buffer,
                0,
                std::mem::size_of::<Entity>() as u64,
            );

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ScreenPicker Render Pass"),
                color_attachments: &[
                    // position
                    Some(wgpu::RenderPassColorAttachment {
                        view: &position_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    // normal
                    Some(wgpu::RenderPassColorAttachment {
                        view: &normal_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    // entity
                    Some(wgpu::RenderPassColorAttachment {
                        view: &entity_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
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

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.set_bind_group(1, &transform_bind_group, &[]);
            render_pass.set_bind_group(2, &material_bind_group, &[]);
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
            render_pass.set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.num_indices() as u32, 0, 0..1);
        }

        let position_buffer = self
            .position_buffer
            .lazy_init(renderer.resource_manager())?;
        let position_buffer = position_buffer.get_buffer().unwrap();

        let normal_buffer = self.normal_buffer.lazy_init(renderer.resource_manager())?;
        let normal_buffer = normal_buffer.get_buffer().unwrap();

        let entity_buffer = self.entity_buffer.lazy_init(renderer.resource_manager())?;
        let entity_buffer = entity_buffer.get_buffer().unwrap();

        let position_texture = self
            .position_texture
            .handle()
            .lazy_init(renderer.resource_manager())?;
        let position_texture = position_texture.get_texture().unwrap();

        let normal_texture = self
            .normal_texture
            .handle()
            .lazy_init(renderer.resource_manager())?;
        let normal_texture = normal_texture.get_texture().unwrap();

        let entity_texture = self
            .entity_texture
            .handle()
            .lazy_init(renderer.resource_manager())?;
        let entity_texture = entity_texture.get_texture().unwrap();

        encoder.copy_texture_to_buffer(
            position_texture.as_image_copy(),
            wgpu::ImageCopyBufferBase {
                buffer: &position_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * 4 * self.screen_width),
                    rows_per_image: None,
                },
            },
            position_texture.size(),
        );

        encoder.copy_texture_to_buffer(
            normal_texture.as_image_copy(),
            wgpu::ImageCopyBufferBase {
                buffer: &normal_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * 4 * self.screen_width),
                    rows_per_image: None,
                },
            },
            normal_texture.size(),
        );

        encoder.copy_texture_to_buffer(
            entity_texture.as_image_copy(),
            wgpu::ImageCopyBufferBase {
                buffer: &entity_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * 2 * self.screen_width),
                    rows_per_image: None,
                },
            },
            entity_texture.size(),
        );

        Ok(())
    }

    pub fn pick(&self, renderer: &Renderer) -> anyhow::Result<Option<PickResult>> {
        let position_buffer = self
            .position_buffer
            .lazy_init(renderer.resource_manager())?;
        let position_buffer = position_buffer.get_buffer().unwrap();

        let normal_buffer = self.normal_buffer.lazy_init(renderer.resource_manager())?;
        let normal_buffer = normal_buffer.get_buffer().unwrap();

        let entity_buffer = self.entity_buffer.lazy_init(renderer.resource_manager())?;
        let entity_buffer = entity_buffer.get_buffer().unwrap();

        let screen_pos = *self.screen_pos.read();
        let v4_offset = (screen_pos.1 * self.screen_width + screen_pos.0) as usize
            * std::mem::size_of::<glam::Vec4>();
        let u32_offset = (screen_pos.1 * self.screen_width + screen_pos.0) as usize
            * std::mem::size_of::<[u32; 2]>();

        let v4_range = v4_offset..(v4_offset + std::mem::size_of::<glam::Vec4>());
        let u32_range = u32_offset..(u32_offset + std::mem::size_of::<[u32; 2]>());

        let (tx, rx1) = crossbeam_channel::bounded(1);
        position_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        renderer.device().poll(wgpu::Maintain::Wait);
        rx1.recv()??;

        let position_view = position_buffer.slice(..).get_mapped_range();
        let position: glam::Vec4 = *bytemuck::from_bytes(&position_view[v4_range.clone()]);
        drop(position_view);

        let (tx, rx2) = crossbeam_channel::bounded(1);
        normal_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        renderer.device().poll(wgpu::Maintain::Wait);
        rx2.recv()??;

        let normal_view = normal_buffer.slice(..).get_mapped_range();
        let normal: glam::Vec4 = *bytemuck::from_bytes(&normal_view[v4_range.clone()]);
        drop(normal_view);

        let (tx, rx3) = crossbeam_channel::bounded(1);
        entity_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        renderer.device().poll(wgpu::Maintain::Wait);
        rx3.recv()??;

        let entity_view = entity_buffer.slice(..).get_mapped_range();
        let entity: [u32; 2] = *bytemuck::from_bytes(&entity_view[u32_range.clone()]);
        drop(entity_view);

        position_buffer.unmap();
        normal_buffer.unmap();
        entity_buffer.unmap();

        let entity = Entity::from_u64(entity[0] as u64 | ((entity[1] as u64) << 32));
        let position = glam::Vec3::new(position.x, position.y, position.z);
        let normal = glam::Vec3::new(normal.x, normal.y, normal.z);

        let normal = normal.normalize_or_zero();
        if normal == glam::Vec3::ZERO {
            return Ok(None);
        }

        Ok(Some(PickResult {
            entity,
            position,
            normal,
            ray: *self.ray.read().as_ref().unwrap(),
        }))
    }
}
