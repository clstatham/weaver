use parking_lot::RwLock;
use weaver_proc_macro::{BindableComponent, GpuComponent};
use wgpu::util::DeviceExt;

use fabricate::prelude::*;

use crate::{
    camera::{Camera, CameraUniform},
    color::Color,
    doodads::{Doodad, Doodads, MAX_DOODADS},
    load_shader,
    renderer::{
        internals::{
            BindGroupLayoutCache, BindableComponent, GpuComponent, GpuResourceType, LazyBindGroup,
            LazyGpuHandle,
        },
        Renderer,
    },
    texture::{DepthTexture, HdrTexture, Texture, TextureFormat},
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

#[rustfmt::skip]
const CUBE_WIREFRAME_INDICES: &[u32] = &[
    // front
    0, 1, 1, 2, 2, 3, 3, 0,
    // back
    4, 5, 5, 6, 6, 7, 7, 4,
    // top
    8, 9, 9, 10, 10, 11, 11, 8,
    // bottom
    12, 13, 13, 14, 14, 15, 15, 12,
    // left
    16, 17, 17, 18, 18, 19, 19, 16,
    // right
    20, 21, 21, 22, 22, 23, 23, 20,
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

#[derive(GpuComponent, BindableComponent)]
#[gpu(update = "update")]
struct DoodadBuffers {
    bind_group: LazyBindGroup<Self>,
    #[storage]
    transform_buffer: LazyGpuHandle,
    #[storage]
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

    fn update(&self, _world: &World) -> anyhow::Result<()> {
        Ok(())
    }
}

pub struct DoodadRenderPass {
    enabled: bool,
    pipeline: wgpu::RenderPipeline,
    wireframe_pipeline: wgpu::RenderPipeline,
    lines: DoodadBuffers,
    cubes: DoodadBuffers,
    wire_cubes: DoodadBuffers,
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
        let line_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Line Vertex Buffer"),
            size: std::mem::size_of::<DoodadVertex>() as u64 * MAX_DOODADS as u64 * 2,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let line_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Line Index Buffer"),
            size: std::mem::size_of::<u32>() as u64 * MAX_DOODADS as u64 * 2,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let cube_vertices = cube_vertices();
        let cube_indices = CUBE_INDICES;
        let cube_wireframe_indices = CUBE_WIREFRAME_INDICES;

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

        let wire_cube_vertex_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Wire Cube Vertex Buffer"),
                contents: bytemuck::cast_slice(&cube_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let wire_cube_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Wire Cube Index Buffer"),
            contents: bytemuck::cast_slice(cube_wireframe_indices),
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

        let shader = device.create_shader_module(load_shader!("doodads.wgsl"));

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
                    format: HdrTexture::FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
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

        let wireframe_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Doodad Wireframe Pipeline"),
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
                    format: HdrTexture::FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                cull_mode: None,
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
            wireframe_pipeline,
            depth_texture,
            camera_buffer,
            lines: DoodadBuffers::new(line_vertex_buffer, line_index_buffer, MAX_DOODADS * 2),
            cubes: DoodadBuffers::new(cube_vertex_buffer, cube_index_buffer, cube_indices.len()),
            wire_cubes: DoodadBuffers::new(
                wire_cube_vertex_buffer,
                wire_cube_index_buffer,
                cube_indices.len(),
            ),
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

    fn resize(&self, renderer: &Renderer, width: u32, height: u32) {
        let manager = &renderer.resource_manager;
        let handle = LazyGpuHandle::new(
            GpuResourceType::Texture {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                format: DepthTexture::FORMAT,
                dimension: wgpu::TextureDimension::D2,
                view_dimension: wgpu::TextureViewDimension::D2,
                width,
                height,
                depth_or_array_layers: 1,
            },
            Some("Doodad Depth Texture"),
            None,
        );
        let handle = handle.lazy_init(manager).unwrap();
        self.depth_texture.handle().reinit(handle);
    }

    fn prepare(&self, world: &World, renderer: &Renderer) -> anyhow::Result<()> {
        let manager = &renderer.resource_manager;
        self.depth_texture.lazy_init(manager)?;
        self.lines.lazy_init(manager)?;
        self.cubes.lazy_init(manager)?;
        self.wire_cubes.lazy_init(manager)?;
        self.cones.lazy_init(manager)?;

        let mut line_vertex_data = Vec::new();
        let mut line_index_data = Vec::new();

        let mut line_colors_handle = self.lines.color_buffer.lazy_init(manager)?;
        let mut cube_colors_handle = self.cubes.color_buffer.lazy_init(manager)?;
        let mut wire_cube_colors_handle = self.wire_cubes.color_buffer.lazy_init(manager)?;
        let mut cone_colors_handle = self.cones.color_buffer.lazy_init(manager)?;

        let mut line_transforms_handle = self.lines.transform_buffer.lazy_init(manager)?;
        let mut cube_transforms_handle = self.cubes.transform_buffer.lazy_init(manager)?;
        let mut wire_cube_transforms_handle =
            self.wire_cubes.transform_buffer.lazy_init(manager)?;
        let mut cone_transforms_handle = self.cones.transform_buffer.lazy_init(manager)?;

        let mut camera_handle = self.camera_buffer.lazy_init(manager)?;
        let camera = world.query().read::<Camera>().unwrap().build();
        let camera = camera.iter().next();
        if camera.is_none() {
            return Ok(());
        }
        let camera = camera.unwrap();
        let camera = camera.get::<Camera>().unwrap();
        camera_handle.update(&[CameraUniform::from(camera)]);

        let mut line_transforms = Vec::new();
        let mut line_colors = Vec::new();

        let mut cube_transforms = Vec::new();
        let mut cube_colors = Vec::new();

        let mut wire_cube_transforms = Vec::new();
        let mut wire_cube_colors = Vec::new();

        let mut cone_transforms = Vec::new();
        let mut cone_colors = Vec::new();

        let doodads = world.read_resource::<Doodads>().unwrap();
        for doodad in doodads.doodads.write().drain(..) {
            match doodad {
                Doodad::Line(line) => {
                    line_vertex_data.push(DoodadVertex {
                        position: line.start,
                        normal: glam::Vec3::ZERO,
                    });
                    line_vertex_data.push(DoodadVertex {
                        position: line.end,
                        normal: glam::Vec3::ZERO,
                    });
                    line_index_data.push(line_vertex_data.len() as u32 - 2);
                    line_index_data.push(line_vertex_data.len() as u32 - 1);
                    line_transforms.push(glam::Mat4::IDENTITY);
                    line_colors.push(line.color);
                }
                Doodad::Cube(cube) => {
                    cube_transforms.push(glam::Mat4::from_scale_rotation_translation(
                        cube.scale,
                        cube.rotation,
                        cube.position,
                    ));
                    cube_colors.push(cube.color);
                }
                Doodad::WireCube(cube) => {
                    wire_cube_transforms.push(glam::Mat4::from_scale_rotation_translation(
                        cube.scale,
                        cube.rotation,
                        cube.position,
                    ));
                    wire_cube_colors.push(cube.color);
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

        renderer.queue().write_buffer(
            &self.lines.vertex_buffer,
            0,
            bytemuck::cast_slice(&line_vertex_data),
        );
        renderer.queue().write_buffer(
            &self.lines.index_buffer,
            0,
            bytemuck::cast_slice(&line_index_data),
        );

        line_transforms_handle.update(&line_transforms);
        line_colors_handle.update(&line_colors);

        cube_transforms_handle.update(&cube_transforms);
        cube_colors_handle.update(&cube_colors);

        wire_cube_transforms_handle.update(&wire_cube_transforms);
        wire_cube_colors_handle.update(&wire_cube_colors);

        cone_transforms_handle.update(&cone_transforms);
        cone_colors_handle.update(&cone_colors);

        *self.lines.count.write() = line_transforms.len();
        *self.cubes.count.write() = cube_transforms.len();
        *self.wire_cubes.count.write() = wire_cube_transforms.len();
        *self.cones.count.write() = cone_transforms.len();

        Ok(())
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_target: &wgpu::TextureView,
        depth_target: &wgpu::TextureView,
        renderer: &crate::renderer::Renderer,
        world: &World,
    ) -> anyhow::Result<()> {
        let manager = &renderer.resource_manager;
        // let depth_texture = &self.depth_texture.handle().lazy_init(manager)?;
        // let depth_texture = depth_texture.get_texture().unwrap();
        // let depth_texture_view = depth_texture.create_view(&Default::default());

        let line_bind_group = self
            .lines
            .lazy_init_bind_group(manager, &renderer.bind_group_layout_cache)?;
        let cube_bind_group = self
            .cubes
            .lazy_init_bind_group(manager, &renderer.bind_group_layout_cache)?;
        let wire_cube_bind_group = self
            .wire_cubes
            .lazy_init_bind_group(manager, &renderer.bind_group_layout_cache)?;
        let cone_bind_group = self
            .cones
            .lazy_init_bind_group(manager, &renderer.bind_group_layout_cache)?;

        let camera = world.query().read::<Camera>().unwrap().build();
        let camera = camera.iter().next();
        if camera.is_none() {
            return Ok(());
        }
        let camera = camera.unwrap();
        let camera = camera.get::<Camera>().unwrap();
        let camera_bind_group =
            camera.lazy_init_bind_group(manager, &renderer.bind_group_layout_cache)?;

        // // clear depth buffer
        // {
        //     let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        //         label: Some("Doodad Render Pass"),
        //         color_attachments: &[],
        //         depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
        //             view: &depth_texture_view,
        //             depth_ops: Some(wgpu::Operations {
        //                 load: wgpu::LoadOp::Clear(1.0),
        //                 store: wgpu::StoreOp::Store,
        //             }),
        //             stencil_ops: None,
        //         }),
        //         timestamp_writes: None,
        //         occlusion_query_set: None,
        //     });
        // }

        // render lines
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

            render_pass.set_pipeline(&self.wireframe_pipeline);
            render_pass.set_bind_group(0, &line_bind_group, &[]);
            render_pass.set_bind_group(1, &camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.lines.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(self.lines.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.lines.index_count as u32, 0, 0..1);
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

        // render wireframe cubes
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

            render_pass.set_pipeline(&self.wireframe_pipeline);
            render_pass.set_bind_group(0, &wire_cube_bind_group, &[]);
            render_pass.set_bind_group(1, &camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.wire_cubes.vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                self.wire_cubes.index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.draw_indexed(
                0..self.wire_cubes.index_count as u32,
                0,
                0..*self.wire_cubes.count.read() as u32,
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
