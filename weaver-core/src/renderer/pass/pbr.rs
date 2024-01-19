use std::sync::Arc;

use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use weaver_proc_macro::{BindableComponent, GpuComponent};

use weaver_ecs::World;

use crate::{
    camera::{Camera, CameraUniform},
    include_shader,
    light::PointLightArray,
    material::Material,
    mesh::{Mesh, Vertex},
    renderer::{
        internals::{
            BindGroupLayoutCache, BindableComponent, GpuComponent, GpuResourceType, LazyBindGroup,
            LazyGpuHandle,
        },
        Renderer,
    },
    texture::{DepthTexture, HdrCubeTexture, HdrTexture, Skybox, Texture, TextureFormat},
    transform::{Transform, TransformArray},
};

use super::sky::{SKYBOX_CUBEMAP_SIZE, SKYBOX_IRRADIANCE_MAP_SIZE};

#[derive(GpuComponent)]
#[gpu(update = "update")]
pub struct UniqueMesh {
    pub mesh: Mesh,
    pub material_bind_group: Arc<wgpu::BindGroup>,
    #[gpu(component)]
    pub transforms: TransformArray,
}

impl UniqueMesh {
    fn update(&self, _world: &World) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Default, GpuComponent)]
#[gpu(update = "update")]
pub struct UniqueMeshes {
    #[gpu(component)]
    pub unique_meshes: FxHashMap<(u64, u64), UniqueMesh>,
}

impl UniqueMeshes {
    pub fn gather(&mut self, world: &World, renderer: &Renderer) {
        let query = world.query::<(&Mesh, &Material, &Transform)>();

        // clear the transforms
        for unique_mesh in self.unique_meshes.values_mut() {
            unique_mesh.transforms.clear();
        }

        for (mesh, material, transform) in query.iter() {
            let unique_mesh = self
                .unique_meshes
                .entry((mesh.asset_id().id(), material.asset_id().id()))
                .or_insert_with(|| UniqueMesh {
                    mesh: mesh.clone(),
                    material_bind_group: material
                        .lazy_init_bind_group(
                            &renderer.resource_manager,
                            &renderer.bind_group_layout_cache,
                        )
                        .unwrap(),
                    transforms: TransformArray::new(),
                });

            unique_mesh.transforms.push(&transform);
        }
    }

    fn update(&self, _world: &World) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Clone, GpuComponent, BindableComponent)]
#[gpu(update = "update")]
pub struct PbrBuffers {
    #[uniform]
    pub(crate) camera: LazyGpuHandle,

    #[texture(format = Rgba32Float, sample_type = float, view_dimension = Cube)]
    #[gpu(component)]
    pub(crate) env_map: Texture,

    #[texture(format = Rgba32Float, sample_type = float, view_dimension = Cube)]
    #[gpu(component)]
    pub(crate) irradiance_map: Texture,

    #[sampler(filtering = false)]
    pub(crate) env_map_sampler: LazyGpuHandle,

    pub(crate) bind_group: LazyBindGroup<Self>,
}

impl PbrBuffers {
    pub fn new() -> Self {
        Self {
            camera: LazyGpuHandle::new(
                GpuResourceType::Uniform {
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    size: std::mem::size_of::<CameraUniform>(),
                },
                Some("PBR Camera"),
                None,
            ),
            env_map: Texture::from_handle(LazyGpuHandle::new(
                GpuResourceType::Texture {
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    format: HdrCubeTexture::FORMAT,
                    width: SKYBOX_CUBEMAP_SIZE,
                    height: SKYBOX_CUBEMAP_SIZE,
                    dimension: wgpu::TextureDimension::D2,
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    depth_or_array_layers: 6,
                },
                Some("PBR Environment Map"),
                None,
            )),
            irradiance_map: Texture::from_handle(LazyGpuHandle::new(
                GpuResourceType::Texture {
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    format: HdrCubeTexture::FORMAT,
                    width: SKYBOX_IRRADIANCE_MAP_SIZE,
                    height: SKYBOX_IRRADIANCE_MAP_SIZE,
                    dimension: wgpu::TextureDimension::D2,
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    depth_or_array_layers: 6,
                },
                Some("PBR Irradiance Map"),
                None,
            )),
            env_map_sampler: LazyGpuHandle::new(
                GpuResourceType::Sampler {
                    address_mode: wgpu::AddressMode::ClampToEdge,
                    filter_mode: wgpu::FilterMode::Nearest,
                    compare: None,
                },
                Some("PBR Environment Map Sampler"),
                None,
            ),
            bind_group: LazyBindGroup::default(),
        }
    }

    fn update(&self, _world: &World) -> anyhow::Result<()> {
        Ok(())
    }
}

impl Default for PbrBuffers {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PbrRenderPass {
    pipeline: wgpu::RenderPipeline,
    buffers: PbrBuffers,
    unique_meshes: RwLock<UniqueMeshes>,
}

impl PbrRenderPass {
    pub fn new(device: &wgpu::Device, bind_group_layout_cache: &BindGroupLayoutCache) -> Self {
        let shader = device.create_shader_module(include_shader!("pbr.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PBR Pipeline Layout"),
            bind_group_layouts: &[
                // mesh transform
                &bind_group_layout_cache.get_or_create::<TransformArray>(device),
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
                        format: HdrTexture::FORMAT,
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
                format: DepthTexture::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let unique_meshes = RwLock::new(UniqueMeshes::default());

        Self {
            pipeline,
            buffers: PbrBuffers::new(),
            unique_meshes,
        }
    }

    pub fn prepare(&self, world: &World, renderer: &Renderer) {
        let mut unique_meshes = self.unique_meshes.write();
        unique_meshes.gather(world, renderer);
        unique_meshes.lazy_init(&renderer.resource_manager).unwrap();
        unique_meshes.update_resources(world).unwrap();
        self.buffers.lazy_init(&renderer.resource_manager).unwrap();
    }

    pub fn render(
        &self,
        renderer: &Renderer,
        hdr_pass_view: &wgpu::TextureView,
        world: &World,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        let skybox = world.query::<&Skybox>();
        let skybox = skybox.iter().next();
        if skybox.is_none() {
            return Ok(());
        }
        let skybox = skybox.unwrap();

        let skybox_handle = &skybox
            .texture
            .handle()
            .lazy_init(&renderer.resource_manager)?;
        let irradiance_handle = &skybox
            .irradiance
            .handle()
            .lazy_init(&renderer.resource_manager)?;
        let skybox_texture = skybox_handle.get_texture().unwrap();
        let irradiance_texture = irradiance_handle.get_texture().unwrap();

        let camera = world.query::<&Camera>();
        let camera = camera.iter().next();
        if camera.is_none() {
            return Ok(());
        }
        let camera = camera.unwrap();

        let camera_handle = camera.handle.lazy_init(&renderer.resource_manager)?;

        let my_camera_buffer = self.buffers.camera.lazy_init(&renderer.resource_manager)?;
        let my_camera_buffer = my_camera_buffer.get_buffer().unwrap();
        let my_env_map_texture = self
            .buffers
            .env_map
            .handle()
            .lazy_init(&renderer.resource_manager)?;
        let my_env_map_texture = my_env_map_texture.get_texture().unwrap();
        let my_irradiance_map_texture = self
            .buffers
            .irradiance_map
            .handle()
            .lazy_init(&renderer.resource_manager)?;
        let my_irradiance_map_texture = my_irradiance_map_texture.get_texture().unwrap();

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

        encoder.copy_texture_to_texture(
            irradiance_texture.as_image_copy(),
            my_irradiance_map_texture.as_image_copy(),
            irradiance_texture.size(),
        );

        let buffer_bind_group = self.buffers.lazy_init_bind_group(
            &renderer.resource_manager,
            &renderer.bind_group_layout_cache,
        )?;

        let point_lights_bind_group = renderer.point_lights.lazy_init_bind_group(
            &renderer.resource_manager,
            &renderer.bind_group_layout_cache,
        )?;

        for unique_mesh in self.unique_meshes.read().unique_meshes.values() {
            let UniqueMesh {
                mesh,
                material_bind_group,
                transforms,
            } = unique_mesh;

            let transform_bind_group = transforms.lazy_init_bind_group(
                &renderer.resource_manager,
                &renderer.bind_group_layout_cache,
            )?;

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
