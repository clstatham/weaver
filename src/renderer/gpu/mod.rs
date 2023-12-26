use rustc_hash::FxHashMap;
use winit::window::Window;

use crate::{
    core::{mesh::Mesh, transform::Transform},
    ecs::{entity::Entity, world::World},
};

use super::Renderer;

struct MeshRenderInfo {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    render_pipeline: wgpu::RenderPipeline,
    transform_bind_group: wgpu::BindGroup,
    transform_buffer: wgpu::Buffer,
    n_indices: usize,
}

pub struct GpuRenderer {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    mesh_render_cache: FxHashMap<Entity, MeshRenderInfo>,
}

impl GpuRenderer {
    pub async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = unsafe { instance.create_surface(window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        Self {
            surface,
            device,
            queue,
            config,
            mesh_render_cache: FxHashMap::default(),
        }
    }

    pub fn render_impl(&mut self, world: &mut World) -> anyhow::Result<()> {
        let output = self.surface.get_current_texture()?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // query the world for meshes and transforms
        let query = world.read::<(Mesh, Transform)>();

        for ((entity, mesh), transform) in query
            .get_with_entity::<Mesh>()
            .into_iter()
            .zip(query.get::<Transform>())
        {
            // check if we already have buffers and a render pipeline for this mesh
            if let Some(info) = self.mesh_render_cache.get(&entity) {
                // update the transform buffer
                self.queue.write_buffer(
                    &info.transform_buffer,
                    0,
                    bytemuck::cast_slice(&[transform.matrix]),
                );
            } else {
                // create buffers and render pipeline
                let (vertex_buffer, index_buffer, render_pipeline) =
                    mesh.create_render_pipeline(&self.device, self.config.format);
                let (transform_bind_group, transform_buffer) =
                    transform.create_bind_group_and_buffer(&self.device);

                self.mesh_render_cache.insert(
                    entity,
                    MeshRenderInfo {
                        vertex_buffer,
                        index_buffer,
                        render_pipeline,
                        transform_bind_group,
                        transform_buffer,
                        n_indices: mesh.indices.len(),
                    },
                );
            }
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            // rasterize each mesh using precalculated info
            for info in self.mesh_render_cache.values() {
                render_pass.set_pipeline(&info.render_pipeline);
                render_pass.set_bind_group(0, &info.transform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, info.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(info.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..info.n_indices as u32, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

impl Renderer for GpuRenderer {
    fn create(window: &winit::window::Window) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        Ok(pollster::block_on(Self::new(window)))
    }
    fn render(&mut self, world: &mut World) -> anyhow::Result<()> {
        self.render_impl(world)
    }
}
