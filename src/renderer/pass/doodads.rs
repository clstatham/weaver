use std::sync::Arc;

use parking_lot::RwLock;
use weaver_proc_macro::Component;
use wgpu::util::DeviceExt;

use crate::{
    core::{
        camera::{Camera, CameraUniform},
        color::Color,
        doodads::{Doodad, Doodads, MAX_DOODADS},
        texture::{DepthTexture, Texture, TextureFormat, WindowTexture},
    },
    ecs::{Query, World},
    include_shader,
    renderer::{
        internals::{
            BindGroupLayoutCache, BindableComponent, GpuComponent, GpuHandle, GpuResourceManager,
            GpuResourceType, LazyBindGroup, LazyGpuHandle,
        },
        Renderer,
    },
};

use super::Pass;

#[rustfmt::skip]
const CUBE_VERTICES: &[glam::Vec3] = &[
    // front
    glam::Vec3::new(-0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, 0.5),
    glam::Vec3::new(-0.5, 0.5, 0.5),
    // back
    glam::Vec3::new(-0.5, -0.5, -0.5),
    glam::Vec3::new(0.5, -0.5, -0.5),
    glam::Vec3::new(0.5, 0.5, -0.5),
    glam::Vec3::new(-0.5, 0.5, -0.5),
    // top
    glam::Vec3::new(-0.5, 0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, -0.5),
    glam::Vec3::new(-0.5, 0.5, -0.5),
    // bottom
    glam::Vec3::new(-0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, -0.5, -0.5),
    glam::Vec3::new(-0.5, -0.5, -0.5),
    // left
    glam::Vec3::new(-0.5, -0.5, 0.5),
    glam::Vec3::new(-0.5, 0.5, 0.5),
    glam::Vec3::new(-0.5, 0.5, -0.5),
    glam::Vec3::new(-0.5, -0.5, -0.5),
    // right
    glam::Vec3::new(0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, -0.5),
    glam::Vec3::new(0.5, -0.5, -0.5),
];

#[rustfmt::skip]
const CUBE_NORMALS: &[glam::Vec3] = &[
    // front
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    // back
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
    // top
    glam::Vec3::new(0.0, 1.0, 0.0),
    glam::Vec3::new(0.0, 1.0, 0.0),
    glam::Vec3::new(0.0, 1.0, 0.0),
    glam::Vec3::new(0.0, 1.0, 0.0),
    // bottom
    glam::Vec3::new(0.0, -1.0, 0.0),
    glam::Vec3::new(0.0, -1.0, 0.0),
    glam::Vec3::new(0.0, -1.0, 0.0),
    glam::Vec3::new(0.0, -1.0, 0.0),
    // left
    glam::Vec3::new(-1.0, 0.0, 0.0),
    glam::Vec3::new(-1.0, 0.0, 0.0),
    glam::Vec3::new(-1.0, 0.0, 0.0),
    glam::Vec3::new(-1.0, 0.0, 0.0),
    // right
    glam::Vec3::new(1.0, 0.0, 0.0),
    glam::Vec3::new(1.0, 0.0, 0.0),
    glam::Vec3::new(1.0, 0.0, 0.0),
    glam::Vec3::new(1.0, 0.0, 0.0),
];

#[rustfmt::skip]
const CUBE_INDICES: &[u32] = &[
    // front
    0, 1, 2, 2, 3, 0,
    // back
    4, 5, 6, 6, 7, 4,
    // top
    8, 9, 10, 10, 11, 8,
    // bottom
    12, 13, 14, 14, 15, 12,
    // left
    16, 17, 18, 18, 19, 16,
    // right
    20, 21, 22, 22, 23, 20,
];

fn cube_vertices() -> Vec<DoodadVertex> {
    let mut vertices = Vec::new();

    for (position, normal) in CUBE_VERTICES.iter().zip(CUBE_NORMALS.iter()) {
        vertices.push(DoodadVertex {
            position: *position,
            normal: *normal,
        });
    }

    vertices
}

fn cone_vertices(n: usize) -> Vec<DoodadVertex> {
    let mut vertices = Vec::new();

    let mut positions = Vec::new();
    let mut normals = Vec::new();

    for i in 0..n {
        let theta = 2.0 * std::f32::consts::PI * (i as f32 / n as f32);
        let x = theta.cos();
        let y = theta.sin();

        positions.push(glam::Vec3::new(x, y, 0.0));
        normals.push(glam::Vec3::new(0.0, 0.0, 1.0));
    }

    positions.push(glam::Vec3::new(0.0, 0.0, 1.0));
    normals.push(glam::Vec3::new(0.0, 0.0, 1.0));

    for (position, normal) in positions.iter().zip(normals.iter()) {
        vertices.push(DoodadVertex {
            position: *position,
            normal: *normal,
        });
    }

    vertices
}

fn cone_indices(n: usize) -> Vec<u32> {
    let mut indices = Vec::new();

    for i in 0..n {
        let i0 = i;
        let i1 = (i + 1) % n;
        let i2 = n;

        indices.push(i0 as u32);
        indices.push(i1 as u32);
        indices.push(i2 as u32);
    }

    indices
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct DoodadVertex {
    position: glam::Vec3,
    normal: glam::Vec3,
}

impl DoodadVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<DoodadVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    format: wgpu::VertexFormat::Float32x3,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<glam::Vec3>() as wgpu::BufferAddress,
                    format: wgpu::VertexFormat::Float32x3,
                    shader_location: 1,
                },
            ],
        }
    }
}

#[derive(Component)]
struct DoodadBuffers {
    bind_group: LazyBindGroup<Self>,
    transform_buffer: LazyGpuHandle,
    color_buffer: LazyGpuHandle,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: usize,
    count: RwLock<usize>,
}

impl DoodadBuffers {
    pub fn new(
        vertex_buffer: wgpu::Buffer,
        index_buffer: wgpu::Buffer,
        index_count: usize,
    ) -> Self {
        Self {
            bind_group: LazyBindGroup::default(),
            transform_buffer: LazyGpuHandle::new(
                GpuResourceType::Storage {
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    size: std::mem::size_of::<glam::Mat4>() * MAX_DOODADS,
                    read_only: true,
                },
                Some("Doodad Transform Buffer"),
                None,
            ),
            color_buffer: LazyGpuHandle::new(
                GpuResourceType::Storage {
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    size: std::mem::size_of::<Color>() * MAX_DOODADS,
                    read_only: true,
                },
                Some("Doodad Color Buffer"),
                None,
            ),
            vertex_buffer,
            index_buffer,
            index_count,
            count: RwLock::new(0),
        }
    }
}

impl GpuComponent for DoodadBuffers {
    fn lazy_init(&self, manager: &GpuResourceManager) -> anyhow::Result<Vec<GpuHandle>> {
        Ok(vec![
            self.transform_buffer.lazy_init(manager)?,
            self.color_buffer.lazy_init(manager)?,
        ])
    }

    fn update_resources(&self, _world: &World) -> anyhow::Result<()> {
        Ok(())
    }

    fn destroy_resources(&self) -> anyhow::Result<()> {
        self.transform_buffer.mark_destroyed();
        self.color_buffer.mark_destroyed();
        Ok(())
    }
}

impl BindableComponent for DoodadBuffers {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Doodad Bind Group Layout"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
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
        let layout = cache.get_or_create::<Self>(manager.device());
        let transform_buffer = self.transform_buffer.lazy_init(manager)?;
        let color_buffer = self.color_buffer.lazy_init(manager)?;
        let bind_group = manager
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Doodad Bind Group"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: transform_buffer.get_buffer().unwrap().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: color_buffer.get_buffer().unwrap().as_entire_binding(),
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

pub struct DoodadRenderPass {
    enabled: bool,
    pipeline: wgpu::RenderPipeline,
    cubes: DoodadBuffers,
    cones: DoodadBuffers,
    depth_texture: DepthTexture,
    camera_buffer: LazyGpuHandle,
}

impl DoodadRenderPass {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        layout_cache: &BindGroupLayoutCache,
    ) -> Self {
        let cube_vertices = cube_vertices();
        let cube_indices = CUBE_INDICES;

        let cube_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cube Vertex Buffer"),
            contents: bytemuck::cast_slice(&cube_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let cube_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cube Index Buffer"),
            contents: bytemuck::cast_slice(cube_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let cone_vertices = cone_vertices(32);
        let cone_indices = cone_indices(32);

        let cone_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cone Vertex Buffer"),
            contents: bytemuck::cast_slice(&cone_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let cone_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cone Index Buffer"),
            contents: bytemuck::cast_slice(&cone_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Doodad Shader"),
            source: wgpu::ShaderSource::Wgsl(include_shader!("doodads.wgsl").into()),
        });

        let camera_buffer = LazyGpuHandle::new(
            GpuResourceType::Uniform {
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                size: std::mem::size_of::<CameraUniform>(),
            },
            Some("Doodad Camera Buffer"),
            None,
        );

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Doodad Pipeline Layout"),
            bind_group_layouts: &[
                &layout_cache.get_or_create::<DoodadBuffers>(device),
                &layout_cache.get_or_create::<Camera>(device),
            ],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Doodad Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[DoodadVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: WindowTexture::FORMAT,
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
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let depth_texture = DepthTexture::from_texture(Texture::new_lazy(
            config.width,
            config.height,
            Some("Doodad Depth Texture"),
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            DepthTexture::FORMAT,
            wgpu::TextureDimension::D2,
            wgpu::TextureViewDimension::D2,
            1,
        ));

        Self {
            enabled: true,
            pipeline,
            depth_texture,
            camera_buffer,
            cubes: DoodadBuffers::new(cube_vertex_buffer, cube_index_buffer, cube_indices.len()),
            cones: DoodadBuffers::new(cone_vertex_buffer, cone_index_buffer, cone_indices.len()),
        }
    }
}

impl Pass for DoodadRenderPass {
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
        let manager = &renderer.resource_manager;
        self.depth_texture.lazy_init(manager)?;
        let mut cubes = self.cubes.lazy_init(manager)?;
        let mut cones = self.cones.lazy_init(manager)?;

        let mut camera_handle = self.camera_buffer.lazy_init(manager)?;
        let camera = Query::<&Camera>::new(world);
        let camera = camera.iter().next().unwrap();
        camera_handle.update(&[CameraUniform::from(&*camera)]);

        let mut cube_transforms = Vec::new();
        let mut cube_colors = Vec::new();

        let mut cone_transforms = Vec::new();
        let mut cone_colors = Vec::new();

        let mut doodads = world.write_resource::<Doodads>()?;
        for doodad in doodads.doodads.drain(..) {
            match doodad {
                Doodad::Cube(cube) => {
                    cube_transforms.push(glam::Mat4::from_scale_rotation_translation(
                        cube.scale,
                        cube.rotation,
                        cube.position,
                    ));
                    cube_colors.push(cube.color);
                }
                Doodad::Cone(cone) => {
                    cone_transforms.push(glam::Mat4::from_scale_rotation_translation(
                        cone.scale,
                        cone.rotation,
                        cone.position,
                    ));
                    cone_colors.push(cone.color);
                }
            }
        }

        cubes[0].update(&cube_transforms);
        cubes[1].update(&cube_colors);

        cones[0].update(&cone_transforms);
        cones[1].update(&cone_colors);

        *self.cubes.count.write() = cube_transforms.len();
        *self.cones.count.write() = cone_transforms.len();

        Ok(())
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_target: &wgpu::TextureView,
        _depth_target: &wgpu::TextureView,
        renderer: &crate::renderer::Renderer,
        world: &World,
    ) -> anyhow::Result<()> {
        let manager = &renderer.resource_manager;
        let depth_texture = &self.depth_texture.lazy_init(manager)?[0];
        let depth_texture = depth_texture.get_texture().unwrap();
        let depth_texture_view = depth_texture.create_view(&Default::default());

        let cube_bind_group = self
            .cubes
            .lazy_init_bind_group(manager, &renderer.bind_group_layout_cache)?;
        let cone_bind_group = self
            .cones
            .lazy_init_bind_group(manager, &renderer.bind_group_layout_cache)?;

        let camera = Query::<&Camera>::new(world);
        let camera = camera.iter().next().unwrap();
        let camera_bind_group =
            camera.lazy_init_bind_group(manager, &renderer.bind_group_layout_cache)?;

        // clear depth buffer
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Doodad Render Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_texture_view,
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

        // render cubes
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Doodad Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &cube_bind_group, &[]);
            render_pass.set_bind_group(1, &camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.cubes.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(self.cubes.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(
                0..self.cubes.index_count as u32,
                0,
                0..*self.cubes.count.read() as u32,
            );
        }

        // render cones
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Doodad Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &cone_bind_group, &[]);
            render_pass.set_bind_group(1, &camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.cones.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(self.cones.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(
                0..self.cones.index_count as u32,
                0,
                0..*self.cones.count.read() as u32,
            );
        }
        Ok(())
    }
}
