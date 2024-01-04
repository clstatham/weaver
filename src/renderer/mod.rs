use std::sync::Arc;

use egui_wgpu::renderer::ScreenDescriptor;
use weaver_proc_macro::Resource;
use winit::window::Window;

use crate::{
    core::{
        texture::{HdrLoader, Texture},
        ui::EguiContext,
    },
    ecs::World,
};

use self::pass::{
    doodads::DoodadRenderPass, hdr::HdrRenderPass, pbr::PbrRenderPass, shadow::ShadowRenderPass,
    sky::SkyRenderPass, Pass,
};

pub mod compute;
pub mod pass;

#[derive(Resource)]
#[allow(dead_code)]
pub struct Renderer {
    pub hdr_loader: HdrLoader,

    pub(crate) surface: wgpu::Surface,
    pub(crate) device: Arc<wgpu::Device>,
    pub(crate) queue: Arc<wgpu::Queue>,
    pub(crate) config: wgpu::SurfaceConfiguration,

    pub(crate) color_texture: Texture,
    pub(crate) depth_texture: Texture,
    pub(crate) normal_texture: Texture,

    pub(crate) hdr_pass: HdrRenderPass,
    pub(crate) pbr_pass: PbrRenderPass,
    pub(crate) sky_pass: SkyRenderPass,
    pub(crate) passes: Vec<Box<dyn pass::Pass>>,

    pub(crate) sampler_clamp_nearest: wgpu::Sampler,
    pub(crate) sampler_clamp_linear: wgpu::Sampler,
    pub(crate) sampler_repeat_nearest: wgpu::Sampler,
    pub(crate) sampler_repeat_linear: wgpu::Sampler,
    pub(crate) sampler_depth: wgpu::Sampler,
}

impl Renderer {
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
                    features: wgpu::Features::all_webgpu_mask(),
                    limits: wgpu::Limits::downlevel_defaults(),
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
            format: Texture::WINDOW_FORMAT,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let color_texture = Texture::create_color_texture(
            &device,
            config.width,
            config.height,
            Some("Color Texture"),
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            Some(Texture::WINDOW_FORMAT),
        );

        let depth_texture = Texture::create_depth_texture(
            &device,
            config.width,
            config.height,
            Some("Depth Texture"),
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
        );

        let normal_texture = Texture::create_normal_texture(
            &device,
            config.width as usize,
            config.height as usize,
            Some("Normal Texture"),
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
        );

        let sampler_clamp_nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Clamp Nearest Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: None,
            ..Default::default()
        });

        let sampler_clamp_linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Clamp Linear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            compare: None,
            ..Default::default()
        });

        let sampler_repeat_nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Repeat Nearest Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: None,
            ..Default::default()
        });

        let sampler_repeat_linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Repeat Linear Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            compare: None,
            ..Default::default()
        });

        let sampler_depth = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Depth Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let hdr_pass =
            HdrRenderPass::new(&device, config.width, config.height, &sampler_clamp_nearest);

        let hdr_loader = HdrLoader::new(&device);

        let skybox = hdr_loader
            .load(&device, &queue, 2048, "assets/meadow_2k.hdr")
            .unwrap();

        let sky_pass = SkyRenderPass::new(&device, skybox, &sampler_clamp_nearest);

        let pbr_pass = PbrRenderPass::new(&device, &sky_pass.bind_group_layout);

        let passes: Vec<Box<dyn pass::Pass>> = vec![
            // shadow pass
            Box::new(ShadowRenderPass::new(
                &device,
                config.width,
                config.height,
                &sampler_clamp_nearest,
                &sampler_depth,
            )),
            // doodad pass
            Box::new(DoodadRenderPass::new(&device, &config)),
        ];

        Self {
            hdr_loader,
            surface,
            device: Arc::new(device),
            queue: Arc::new(queue),
            config,
            color_texture,
            depth_texture,
            normal_texture,
            hdr_pass,
            pbr_pass,
            sky_pass,
            passes,
            sampler_clamp_nearest,
            sampler_clamp_linear,
            sampler_repeat_nearest,
            sampler_repeat_linear,
            sampler_depth,
        }
    }

    pub fn push_render_pass<T: Pass + 'static>(&mut self, pass: T) {
        self.passes.push(Box::new(pass));
    }

    pub fn prepare_components(&self, world: &World) {
        self.pbr_pass.prepare_components(world, self);
    }

    pub fn render_ui(&self, ui: &mut EguiContext, window: &Window, output: &wgpu::SurfaceTexture) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render UI Encoder"),
            });

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        ui.render(
            &self.device,
            &self.queue,
            &mut encoder,
            window,
            &view,
            &ScreenDescriptor {
                size_in_pixels: [self.config.width, self.config.height],
                pixels_per_point: window.scale_factor() as f32,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    pub fn render(&self, world: &World, output: &wgpu::SurfaceTexture) -> anyhow::Result<()> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        // clear the screen
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Screen"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: self.hdr_pass.texture.view(),
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: self.normal_texture.view(),
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: self.depth_texture.view(),
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

        self.queue.submit(std::iter::once(encoder.finish()));

        self.pbr_pass.render(
            &self.device,
            &self.queue,
            &self.hdr_pass.texture,
            &self.depth_texture,
            &self.normal_texture,
            &self.sky_pass.bind_group,
            world,
        )?;

        for pass in self.passes.iter() {
            pass.render(
                &self.device,
                &self.queue,
                &self.hdr_pass.texture,
                &self.depth_texture,
                world,
            )?;
        }

        self.sky_pass.render(
            &self.device,
            &self.queue,
            &self.hdr_pass.texture,
            &self.depth_texture,
            world,
        )?;

        self.hdr_pass.render(
            &self.device,
            &self.queue,
            &self.color_texture,
            &self.depth_texture,
            world,
        )?;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Copy Color Texture Encoder"),
            });

        // copy color texture to the output
        encoder.copy_texture_to_texture(
            self.color_texture.texture().as_image_copy(),
            output.texture.as_image_copy(),
            wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    pub fn prepare(&self) -> wgpu::SurfaceTexture {
        self.surface.get_current_texture().unwrap()
    }

    pub fn present(&self, output: wgpu::SurfaceTexture) {
        output.present();
    }
}
