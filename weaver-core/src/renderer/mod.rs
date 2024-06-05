use std::{borrow::Cow, io::Read, rc::Rc, sync::Arc};

use egui_wgpu::renderer::ScreenDescriptor;
use naga_oil::compose::{ComposableModuleDescriptor, Composer, NagaModuleDescriptor};
use parking_lot::RwLock;

use crate::{
    app::Window,
    camera::Camera,
    ecs::{query::Query, world::World},
    geom::Rect,
    light::{PointLight, PointLightArray},
    material::Material,
    prelude::Scene,
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
    viewport::Viewport,
};

pub mod compute;
pub mod internals;
pub mod pass;
pub mod viewport;

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

#[allow(dead_code)]
pub struct Renderer {
    surface: wgpu::Surface,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    config: Arc<RwLock<wgpu::SurfaceConfiguration>>,

    main_viewport: Arc<RwLock<Viewport>>,

    hdr_pass: HdrRenderPass,
    pbr_pass: PbrRenderPass,
    sky_pass: SkyRenderPass,
    shadow_pass: OmniShadowRenderPass,
    doodad_pass: DoodadRenderPass,
    extra_passes: Vec<Box<dyn pass::Pass>>,

    resource_manager: Arc<GpuResourceManager>,
    bind_group_layout_cache: BindGroupLayoutCache,

    point_lights: PointLightArray,
    world: Rc<World>,
    output: Arc<RwLock<Option<wgpu::SurfaceTexture>>>,
}

impl Clone for Renderer {
    fn clone(&self) -> Self {
        unimplemented!("Renderer is not cloneable")
    }
}

impl Renderer {
    pub fn new(vsync: bool, window: &winit::window::Window, world: Rc<World>) -> Self {
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
            present_mode: if vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            },
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

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

        let main_viewport = Viewport::new(
            Rect::new(0.0, 0.0, config.width as f32, config.height as f32),
            &device,
            &bind_group_layout_cache,
        );

        Self {
            surface,
            device,
            queue,
            config: Arc::new(RwLock::new(config)),
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
            main_viewport: Arc::new(RwLock::new(main_viewport)),
        }
    }

    pub fn screen_size(&self) -> glam::Vec2 {
        glam::Vec2::new(
            self.config.read().width as f32,
            self.config.read().height as f32,
        )
    }

    pub fn viewport_rect(&self) -> Rect {
        self.main_viewport.read().rect
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

    pub fn main_viewport(&self) -> &Arc<RwLock<Viewport>> {
        &self.main_viewport
    }

    pub fn set_viewport_rect(&self, rect: Rect) {
        if rect == self.main_viewport.read().rect {
            return;
        }
        self.move_viewport(rect.x, rect.y);
        self.resize_viewport(rect.width as u32, rect.height as u32);
    }

    pub fn set_viewport_enabled(&self, enabled: bool) {
        self.main_viewport.write().enabled = enabled;
    }

    pub fn viewport_enabled(&self) -> bool {
        self.main_viewport.read().enabled
    }

    pub fn resize_viewport(&self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if width == self.main_viewport.read().rect.width as u32
            && height == self.main_viewport.read().rect.height as u32
        {
            return;
        }

        log::debug!("Resizing viewport to {}x{}", width, height);

        self.main_viewport.write().resize(self, width, height);

        self.pbr_pass.resize(self, width, height);
        self.sky_pass.resize(self, width, height);
        self.hdr_pass.resize(self, width, height);
        self.shadow_pass.resize(self, width, height);
        self.doodad_pass.resize(self, width, height);

        for pass in self.extra_passes.iter() {
            pass.resize(self, width, height);
        }

        self.resource_manager().update_all_resources();
        self.force_flush();
    }

    pub fn move_viewport(&self, x: f32, y: f32) {
        self.main_viewport.write().move_to(x, y);
    }

    pub fn resize_surface(&self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if width == self.config.read().width && height == self.config.read().height {
            return;
        }

        // discard the current output texture
        self.output.write().take();

        log::debug!("Resizing renderer to {}x{}", width, height);
        self.config.write().width = width;
        self.config.write().height = height;

        let config = &*self.config.read();

        self.surface.configure(&self.device, config);

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

        let resource_manager = &self.resource_manager;
        // prepare the renderer's built-in components
        self.hdr_pass.texture.lazy_init(resource_manager).unwrap();

        {
            let query = self.world.query(&Query::new().read::<Material>());
            for entity in query.iter() {
                let material = query.get::<Material>(entity).unwrap();
                material.lazy_init(resource_manager).unwrap();
                material.update_resources(&self.world).unwrap();
            }
        }

        {
            self.point_lights.clear();

            let query = self.world.query(&Query::new().read::<PointLight>());
            for entity in query.iter() {
                let light = query.get::<PointLight>(entity).unwrap();
                light.lazy_init(resource_manager).unwrap();
                light.update_resources(&self.world).unwrap();
                self.point_lights.add_light(&light);
            }

            self.point_lights.update_resources(&self.world).unwrap();
        }

        {
            let query = self.world.query(&Query::new().read::<Camera>());
            for entity in query.iter() {
                let camera = query.get::<Camera>(entity).unwrap();
                camera.lazy_init(resource_manager).unwrap();
                camera.update_resources(&self.world).unwrap();
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
                &window.window,
                &view,
                &ScreenDescriptor {
                    size_in_pixels: [self.config.read().width, self.config.read().height],
                    pixels_per_point: window.window.scale_factor() as f32,
                },
            );

            // self.set_viewport_rect(ui.state().read().egui_ctx().available_rect().into());
        }
    }

    pub fn render_to_viewport(&mut self, encoder: &mut wgpu::CommandEncoder) -> anyhow::Result<()> {
        let viewport_view = {
            let viewport_handle = &self
                .main_viewport
                .read()
                .hdr_pass
                .texture
                .handle()
                .lazy_init(&self.resource_manager)?;
            let viewport_texture = viewport_handle.get_texture().unwrap();
            viewport_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Viewport Texture View"),
                format: Some(HdrTexture::FORMAT),
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                base_array_layer: 0,
                array_layer_count: None,
                mip_level_count: None,
            })
        };

        let viewport_depth_view = {
            let viewport_handle = &self
                .main_viewport
                .read()
                .depth_texture
                .handle()
                .lazy_init(&self.resource_manager)?;
            let viewport_texture = viewport_handle.get_texture().unwrap();
            viewport_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Viewport Depth Texture View"),
                format: Some(DepthTexture::FORMAT),
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                base_array_layer: 0,
                array_layer_count: None,
                mip_level_count: None,
            })
        };

        // clear the viewports
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Screen"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &viewport_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &viewport_depth_view,
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

        self.pbr_pass.render(
            self,
            &viewport_view,
            &viewport_depth_view,
            &self.world,
            encoder,
        )?;

        for pass in self.extra_passes.iter() {
            pass.render_if_enabled(
                encoder,
                &viewport_view,
                &viewport_depth_view,
                self,
                &self.world,
            )?;
        }

        self.sky_pass.render_if_enabled(
            encoder,
            &viewport_view,
            &viewport_depth_view,
            self,
            &self.world,
        )?;

        self.doodad_pass.render_if_enabled(
            encoder,
            &viewport_view,
            &viewport_depth_view,
            self,
            &self.world,
        )?;

        // self.shadow_pass.render_if_enabled(
        //     encoder,
        //     &self.color_texture_view.read(),
        //     &self.depth_texture_view.read(),
        //     self,
        //     world,
        // )?;

        // self.particle_pass.render_if_enabled(
        //     &self.device,
        //     &self.queue,
        //     &self.color_texture_view,
        //     &self.depth_texture_view,
        //     self,
        //     world,
        // )?;

        self.main_viewport
            .read()
            .render(encoder, self, &self.world)?;

        Ok(())
    }

    pub fn begin_render(&mut self) -> wgpu::CommandEncoder {
        log::trace!("Begin frame");

        let output = self
            .output
            .write()
            .take()
            .unwrap_or_else(|| self.surface.get_current_texture().unwrap());

        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Main Render Encoder"),
            });

        *self.output.write() = Some(output);

        encoder
    }

    pub fn prepare_passes(&mut self) {
        log::trace!("Preparing passes");
        self.pbr_pass.prepare(&self.world, self);
        self.shadow_pass
            .prepare_if_enabled(&self.world, self)
            .unwrap();
        self.doodad_pass
            .prepare_if_enabled(&self.world, self)
            .unwrap();
        self.sky_pass.prepare_if_enabled(&self.world, self).unwrap();
        self.hdr_pass.prepare(&self.world, self).unwrap();

        for pass in self.extra_passes.iter() {
            pass.prepare_if_enabled(&self.world, self).unwrap();
        }

        self.resource_manager.update_all_resources();
    }

    pub fn end_render(&self, encoder: wgpu::CommandEncoder) {
        self.flush(encoder);
        self.resource_manager.gc_destroyed_resources();
    }

    pub fn present(&self) {
        if let Some(Some(output)) = self.output.try_write().map(|mut o| o.take()) {
            output.present();
        }
    }
}

pub fn render_system(scene: &Scene) -> anyhow::Result<()> {
    let world = scene.world();
    let mut ui = world.get_resource_mut::<EguiContext>().unwrap();
    let window = world.get_resource::<Window>().unwrap();
    let mut renderer = world.get_resource_mut::<Renderer>().unwrap();
    let mut encoder = renderer.begin_render();

    renderer.prepare_components();
    renderer.prepare_passes();

    renderer.render_to_viewport(&mut encoder)?;
    renderer.render_ui(&mut ui, &window, &mut encoder);

    renderer.end_render(encoder);

    Ok(())
}
