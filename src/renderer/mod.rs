use std::sync::Arc;

use egui_wgpu::renderer::ScreenDescriptor;
use weaver_proc_macro::Resource;
use winit::window::Window;

use crate::{
    core::{
        camera::Camera,
        light::{PointLight, PointLightArray},
        material::Material,
        texture::{
            DepthTexture, HdrTexture, NormalMapTexture, PositionMapTexture, TextureFormat,
            WindowTexture,
        },
        ui::EguiContext,
    },
    ecs::{Query, World},
    renderer::internals::GpuComponent,
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

#[derive(Resource)]
#[allow(dead_code)]
pub struct Renderer {
    surface: wgpu::Surface,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    config: wgpu::SurfaceConfiguration,

    color_texture: wgpu::Texture,
    color_texture_view: wgpu::TextureView,
    depth_texture: wgpu::Texture,
    depth_texture_view: wgpu::TextureView,
    position_texture: wgpu::Texture,
    position_texture_view: wgpu::TextureView,
    normal_texture: wgpu::Texture,
    normal_texture_view: wgpu::TextureView,

    pub hdr_pass: HdrRenderPass,
    pub pbr_pass: PbrRenderPass,
    pub sky_pass: SkyRenderPass,
    pub shadow_pass: OmniShadowRenderPass,
    pub doodad_pass: DoodadRenderPass,
    pub extra_passes: Vec<Box<dyn pass::Pass>>,

    resource_manager: Arc<GpuResourceManager>,
    bind_group_layout_cache: BindGroupLayoutCache,

    point_lights: PointLightArray,
}

impl Renderer {
    pub fn new(window: &Window) -> Self {
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
                features: wgpu::Features::all_webgpu_mask() | wgpu::Features::MULTIVIEW,
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
            present_mode: wgpu::PresentMode::AutoNoVsync,
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

        let position_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Position Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PositionMapTexture::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let position_texture_view = position_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Position Texture View"),
            format: Some(PositionMapTexture::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
        });

        let normal_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Normal Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: NormalMapTexture::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let normal_texture_view = normal_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Normal Texture View"),
            format: Some(NormalMapTexture::FORMAT),
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
            config,
            color_texture,
            color_texture_view,
            depth_texture,
            depth_texture_view,
            position_texture,
            position_texture_view,
            normal_texture,
            normal_texture_view,
            hdr_pass,
            pbr_pass,
            shadow_pass,
            sky_pass,
            doodad_pass,
            extra_passes,
            resource_manager,
            bind_group_layout_cache,
            point_lights,
        }
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

    pub fn prepare_components(&mut self, world: &World) {
        log::trace!("Preparing components");

        let resource_manager = &self.resource_manager;
        // prepare the renderer's built-in components
        self.hdr_pass.texture.lazy_init(resource_manager).unwrap();

        // query the world for the types that need to allocate buffers
        // these are currently:
        // - Material
        // - PointLight
        // - Camera

        {
            let query = Query::<&Material>::new(world);
            for material in query.iter() {
                material.lazy_init(resource_manager).unwrap();
                material.update_resources(world).unwrap();
            }
        }

        {
            self.point_lights.clear();

            let query = Query::<&PointLight>::new(world);
            for light in query.iter() {
                light.lazy_init(resource_manager).unwrap();
                light.update_resources(world).unwrap();
                self.point_lights.add_light(&light);
            }

            self.point_lights.update_resources(world).unwrap();
        }

        {
            let query = Query::<&Camera>::new(world);
            for camera in query.iter() {
                camera.lazy_init(resource_manager).unwrap();
                camera.update_resources(world).unwrap();
            }
        }

        self.resource_manager.update_all_resources();
    }

    pub fn render_ui(
        &self,
        ui: &mut EguiContext,
        window: &Window,
        output: &wgpu::SurfaceTexture,
        encoder: &mut wgpu::CommandEncoder,
    ) {
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
                size_in_pixels: [self.config.width, self.config.height],
                pixels_per_point: window.scale_factor() as f32,
            },
        );
    }

    pub fn render(
        &mut self,
        world: &World,
        output: &wgpu::SurfaceTexture,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        let hdr_pass_view = {
            let hdr_pass_handle = &self.hdr_pass.texture.lazy_init(&self.resource_manager)?;
            let hdr_pass_texture = hdr_pass_handle[0].get_texture().unwrap();
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
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Screen"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &hdr_pass_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.normal_texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.position_texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture_view,
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
                &self.depth_texture_view,
                self,
                world,
            )?;
        }

        self.sky_pass.render_if_enabled(
            encoder,
            &hdr_pass_view,
            &self.depth_texture_view,
            self,
            world,
        )?;

        // we always want to render the HDR pass, otherwise we won't see anything!
        self.hdr_pass.render(
            encoder,
            &self.color_texture_view,
            &self.depth_texture_view,
            self,
            world,
        )?;

        self.shadow_pass.render_if_enabled(
            encoder,
            &self.color_texture_view,
            &self.depth_texture_view,
            self,
            world,
        )?;

        self.doodad_pass.render_if_enabled(
            encoder,
            &self.color_texture_view,
            &self.depth_texture_view,
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
            self.color_texture.as_image_copy(),
            output.texture.as_image_copy(),
            wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    pub fn begin_frame(&mut self) -> (wgpu::SurfaceTexture, wgpu::CommandEncoder) {
        log::trace!("Begin frame");

        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Main Render Encoder"),
            });

        let output = self.surface.get_current_texture().unwrap();

        (output, encoder)
    }

    pub fn prepare_passes(&mut self, world: &World) {
        log::trace!("Preparing passes");
        self.pbr_pass.prepare(world, self);
        self.shadow_pass.prepare(world, self).unwrap();
        self.doodad_pass.prepare(world, self).unwrap();
        self.sky_pass.prepare(world, self).unwrap();
        self.hdr_pass.prepare(world, self).unwrap();
        for pass in self.extra_passes.iter() {
            pass.prepare(world, self).unwrap();
        }
        self.resource_manager.update_all_resources();
    }

    pub fn flush_and_present(&self, output: wgpu::SurfaceTexture, encoder: wgpu::CommandEncoder) {
        self.flush(encoder);
        output.present();
        self.resource_manager.gc_destroyed_resources();
    }
}
