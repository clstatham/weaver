use std::{borrow::Cow, io::Read, sync::Arc};

use egui_wgpu::renderer::ScreenDescriptor;
use naga_oil::compose::{ComposableModuleDescriptor, Composer, NagaModuleDescriptor};
use parking_lot::RwLock;
use winit::window::Window;

use weaver_ecs::prelude::*;

use crate::{
    camera::Camera,
    light::{PointLight, PointLightArray},
    material::Material,
    renderer::internals::GpuComponent,
    texture::{DepthTexture, HdrTexture, TextureFormat, WindowTexture},
    ui::EguiContext,
};

use self::{
    internals::{BindGroupLayoutCache, GpuResourceManager},
    pass::{
        doodads::DoodadRenderPass, hdr::HdrRenderPass, pbr::PbrRenderPass,
        shadow::OmniShadowRenderPass, sky::SkyRenderPass, Pass,
    },
};

pub mod compute;
pub mod internals;
pub mod pass;

fn try_every_shader_file(
    composer: &mut Composer,
    for_shader: &str,
    shader_dir: &str,
    max_iters: usize,
) -> anyhow::Result<()> {
    let mut try_again = true;
    let mut iters = 0;
    while try_again {
        try_again = false;
        let shader_dir = std::fs::read_dir(shader_dir)?;
        for entry in shader_dir {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if path.extension().unwrap() != "wgsl" {
                    continue;
                }
                if path.to_str().unwrap() == for_shader {
                    continue;
                }

                let mut file = std::fs::File::open(&path)?;
                let mut shader = String::new();

                file.read_to_string(&mut shader)?;

                if composer
                    .add_composable_module(ComposableModuleDescriptor {
                        file_path: path.to_str().unwrap(),
                        source: shader.as_str(),
                        ..Default::default()
                    })
                    .is_err()
                {
                    try_again = true;
                }
            } else if path.is_dir() {
                try_every_shader_file(composer, for_shader, path.to_str().unwrap(), max_iters)?;
            }
        }

        iters += 1;

        if iters > max_iters {
            return Err(anyhow::anyhow!("Max iterations reached"));
        }
    }

    Ok(())
}

pub fn preprocess_shader(
    file_path: &'static str,
    base_include_path: &'static str,
) -> wgpu::ShaderModuleDescriptor<'static> {
    let mut composer = Composer::non_validating();

    let shader = std::fs::read_to_string(file_path).unwrap();

    try_every_shader_file(&mut composer, file_path, base_include_path, 100).unwrap();

    let module = composer
        .make_naga_module(NagaModuleDescriptor {
            file_path,
            source: shader.as_str(),
            ..Default::default()
        })
        .unwrap_or_else(|e| {
            log::error!("Failed to compile shader {}: {}", file_path, e.inner);
            panic!("{}", e.inner);
        });

    wgpu::ShaderModuleDescriptor {
        label: Some(file_path),
        source: wgpu::ShaderSource::Naga(Cow::Owned(module)),
    }
}

#[macro_export]
macro_rules! load_shader {
    ($file_path:literal) => {
        $crate::renderer::preprocess_shader(
            concat!("assets/shaders/", $file_path),
            "assets/shaders",
        )
    };
}

#[derive(Component)]
#[allow(dead_code)]
pub struct Renderer {
    surface: wgpu::Surface,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    config: Arc<RwLock<wgpu::SurfaceConfiguration>>,

    color_texture: Arc<RwLock<wgpu::Texture>>,
    color_texture_view: Arc<RwLock<wgpu::TextureView>>,
    depth_texture: Arc<RwLock<wgpu::Texture>>,
    depth_texture_view: Arc<RwLock<wgpu::TextureView>>,

    hdr_pass: HdrRenderPass,
    pbr_pass: PbrRenderPass,
    sky_pass: SkyRenderPass,
    shadow_pass: OmniShadowRenderPass,
    doodad_pass: DoodadRenderPass,
    extra_passes: Vec<Box<dyn pass::Pass>>,

    resource_manager: Arc<GpuResourceManager>,
    bind_group_layout_cache: BindGroupLayoutCache,

    point_lights: PointLightArray,
    world: Arc<RwLock<World>>,
    output: Arc<RwLock<Option<wgpu::SurfaceTexture>>>,
}

impl Renderer {
    pub fn new(window: &Window, world: Arc<RwLock<World>>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = unsafe { instance.create_surface(window) }.unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::all_webgpu_mask()
                    | wgpu::Features::MULTIVIEW
                    | wgpu::Features::VERTEX_WRITABLE_STORAGE,
                limits: wgpu::Limits::downlevel_defaults(),
            },
            None,
        ))
        .unwrap();

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_caps = surface.get_capabilities(&adapter);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
            format: WindowTexture::FORMAT,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Color Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: WindowTexture::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let color_texture_view = color_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Color Texture View"),
            format: Some(WindowTexture::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
        });

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DepthTexture::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Depth Texture View"),
            format: Some(DepthTexture::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
        });

        let resource_manager = GpuResourceManager::new(device.clone(), queue.clone());

        let bind_group_layout_cache = BindGroupLayoutCache::default();

        let hdr_pass = HdrRenderPass::new(
            &device,
            config.width,
            config.height,
            &bind_group_layout_cache,
        );

        let pbr_pass = PbrRenderPass::new(&device, &bind_group_layout_cache);

        let sky_pass = SkyRenderPass::new(&device, &bind_group_layout_cache);

        let shadow_pass = OmniShadowRenderPass::new(&device, &bind_group_layout_cache);

        let doodad_pass = DoodadRenderPass::new(&device, &config, &bind_group_layout_cache);

        let extra_passes: Vec<Box<dyn Pass>> = vec![];

        let point_lights = PointLightArray::new();

        Self {
            surface,
            device,
            queue,
            config: Arc::new(RwLock::new(config)),
            color_texture: Arc::new(RwLock::new(color_texture)),
            color_texture_view: Arc::new(RwLock::new(color_texture_view)),
            depth_texture: Arc::new(RwLock::new(depth_texture)),
            depth_texture_view: Arc::new(RwLock::new(depth_texture_view)),
            hdr_pass,
            pbr_pass,
            shadow_pass,
            sky_pass,
            doodad_pass,
            extra_passes,
            resource_manager,
            bind_group_layout_cache,
            point_lights,
            world,
            output: Arc::new(RwLock::new(None)),
        }
    }

    pub fn screen_size(&self) -> glam::Vec2 {
        glam::Vec2::new(
            self.config.read().width as f32,
            self.config.read().height as f32,
        )
    }

    pub fn device(&self) -> &Arc<wgpu::Device> {
        &self.device
    }

    pub fn queue(&self) -> &Arc<wgpu::Queue> {
        &self.queue
    }

    pub fn resource_manager(&self) -> &Arc<GpuResourceManager> {
        &self.resource_manager
    }

    pub fn resize(&self, width: u32, height: u32) {
        // discard the current output texture
        self.output.write().take();

        log::debug!("Resizing renderer to {}x{}", width, height);
        self.config.write().width = width;
        self.config.write().height = height;

        let config = &*self.config.read();

        self.surface.configure(&self.device, config);

        *self.color_texture.write() = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Color Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: WindowTexture::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        *self.color_texture_view.write() =
            self.color_texture
                .read()
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Color Texture View"),
                    format: Some(WindowTexture::FORMAT),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    base_array_layer: 0,
                    array_layer_count: None,
                    mip_level_count: None,
                });

        *self.depth_texture.write() = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DepthTexture::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        *self.depth_texture_view.write() =
            self.depth_texture
                .read()
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Depth Texture View"),
                    format: Some(DepthTexture::FORMAT),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    base_array_layer: 0,
                    array_layer_count: None,
                    mip_level_count: None,
                });

        self.hdr_pass.resize(self, width, height);
        self.pbr_pass.resize(self, width, height);
        self.sky_pass.resize(self, width, height);
        self.shadow_pass.resize(self, width, height);
        self.doodad_pass.resize(self, width, height);

        for pass in self.extra_passes.iter() {
            pass.resize(self, width, height);
        }

        self.resource_manager.update_all_resources();
        self.force_flush();
    }

    /// Forces the render queue to flush, submitting an empty encoder.
    pub fn force_flush(&self) {
        log::trace!("Forcing flush of render queue");
        self.queue.submit(std::iter::once(
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Force Flush Encoder"),
                })
                .finish(),
        ));
    }

    /// Flushes the render queue, submitting the given encoder.
    pub fn flush(&self, encoder: wgpu::CommandEncoder) {
        log::trace!("Flushing render queue");
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    pub fn push_render_pass<T: Pass + 'static>(&mut self, pass: T) {
        self.extra_passes.push(Box::new(pass));
    }

    pub fn prepare_components(&mut self) {
        log::trace!("Preparing components");

        let world = &self.world.read();
        let resource_manager = &self.resource_manager;
        // prepare the renderer's built-in components
        self.hdr_pass.texture.lazy_init(resource_manager).unwrap();

        // query the world for the components that need to allocate resources
        // these are currently:
        // - Material
        // - PointLight
        // - Camera

        {
            let query = world.query::<&Material>();
            for material in query.iter() {
                material.lazy_init(resource_manager).unwrap();
                material.update_resources(world).unwrap();
            }
        }

        {
            self.point_lights.clear();

            let query = world.query::<&PointLight>();
            for light in query.iter() {
                light.lazy_init(resource_manager).unwrap();
                light.update_resources(world).unwrap();
                self.point_lights.add_light(&light);
            }

            self.point_lights.update_resources(world).unwrap();
        }

        {
            let query = world.query::<&Camera>();
            for camera in query.iter() {
                camera.lazy_init(resource_manager).unwrap();
                camera.update_resources(world).unwrap();
            }
        }

        self.resource_manager.update_all_resources();
    }

    pub fn render_ui(
        &mut self,
        ui: &mut EguiContext,
        window: &Window,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if let Some(output) = self.output.read().as_ref() {
            let view = output.texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("UI Texture View"),
                format: Some(WindowTexture::FORMAT),
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                base_array_layer: 0,
                array_layer_count: None,
                mip_level_count: None,
            });

            ui.render(
                &self.device,
                &self.queue,
                encoder,
                window,
                &view,
                &ScreenDescriptor {
                    size_in_pixels: [self.config.read().width, self.config.read().height],
                    pixels_per_point: window.scale_factor() as f32,
                },
            );
        }
    }

    pub fn render(&mut self, encoder: &mut wgpu::CommandEncoder) -> anyhow::Result<()> {
        if let Some(output) = self.output.read().as_ref() {
            let world = &self.world.read();
            let hdr_pass_view = {
                let hdr_pass_handle = &self
                    .hdr_pass
                    .texture
                    .handle()
                    .lazy_init(&self.resource_manager)?;
                let hdr_pass_texture = hdr_pass_handle.get_texture().unwrap();
                hdr_pass_texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("HDR Pass Texture View"),
                    format: Some(HdrTexture::FORMAT),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    base_array_layer: 0,
                    array_layer_count: None,
                    mip_level_count: None,
                })
            };

            // clear the screen
            {
                let depth_view = self.depth_texture_view.read();
                let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Clear Screen"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &hdr_pass_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
            }

            self.pbr_pass.render(self, &hdr_pass_view, world, encoder)?;

            for pass in self.extra_passes.iter() {
                pass.render_if_enabled(
                    encoder,
                    &hdr_pass_view,
                    &self.depth_texture_view.read(),
                    self,
                    world,
                )?;
            }

            self.sky_pass.render_if_enabled(
                encoder,
                &hdr_pass_view,
                &self.depth_texture_view.read(),
                self,
                world,
            )?;

            // we always want to render the HDR pass, otherwise we won't see anything!
            self.hdr_pass.render(
                encoder,
                &self.color_texture_view.read(),
                &self.depth_texture_view.read(),
                self,
                world,
            )?;

            self.shadow_pass.render_if_enabled(
                encoder,
                &self.color_texture_view.read(),
                &self.depth_texture_view.read(),
                self,
                world,
            )?;

            self.doodad_pass.render_if_enabled(
                encoder,
                &self.color_texture_view.read(),
                &self.depth_texture_view.read(),
                self,
                world,
            )?;

            // self.particle_pass.render_if_enabled(
            //     &self.device,
            //     &self.queue,
            //     &self.color_texture_view,
            //     &self.depth_texture_view,
            //     self,
            //     world,
            // )?;

            // copy color texture to the output
            encoder.copy_texture_to_texture(
                self.color_texture.read().as_image_copy(),
                output.texture.as_image_copy(),
                wgpu::Extent3d {
                    width: self.config.read().width,
                    height: self.config.read().height,
                    depth_or_array_layers: 1,
                },
            );
        }

        Ok(())
    }

    pub fn begin_frame(&mut self) -> Option<wgpu::CommandEncoder> {
        if self.output.read().is_some() {
            return None;
        }
        log::trace!("Begin frame");

        // return early if we can't render for whatever reason
        let output = self.surface.get_current_texture().ok()?;

        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Main Render Encoder"),
            });

        *self.output.write() = Some(output);

        Some(encoder)
    }

    pub fn prepare_passes(&mut self) {
        log::trace!("Preparing passes");
        let world = &self.world.read();

        self.pbr_pass.prepare(world, self);
        self.shadow_pass.prepare_if_enabled(world, self).unwrap();
        self.doodad_pass.prepare_if_enabled(world, self).unwrap();
        self.sky_pass.prepare_if_enabled(world, self).unwrap();
        self.hdr_pass.prepare(world, self).unwrap();

        for pass in self.extra_passes.iter() {
            pass.prepare_if_enabled(world, self).unwrap();
        }

        self.resource_manager.update_all_resources();
    }

    pub fn end_frame(&self, encoder: wgpu::CommandEncoder) {
        self.flush(encoder);
        self.resource_manager.gc_destroyed_resources();
    }

    pub fn present(&self) {
        if let Some(Some(output)) = self.output.try_write().map(|mut o| o.take()) {
            output.present();
        }
    }
}
