//! The window surface and the per-frame swapchain texture.
//!
//! Frame lifecycle, all on the render thread:
//! 1. [`RenderStage::Prepare`]: [`resize_surface`] reconfigures the surface when the window was
//!    resized, then [`acquire_frame`] acquires the next swapchain texture. Only if that succeeds
//!    are [`SurfaceFrame`] and [`ActiveCommandEncoder`] inserted.
//! 2. [`RenderStage::Render`]: draw systems record into the encoder. They are gated on
//!    [`frame_in_flight`], so they simply don't run on frames where no texture could be acquired
//!    (minimized window, outdated surface, timeout).
//! 3. [`RenderStage::Present`]: [`present_frame`] submits everything and presents.

use weaver_ecs::{
    commands::Commands,
    component::{Res, ResMut},
    condition::{Condition, resource_exists},
    world::World,
};
use weaver_event::EventRx;
use weaver_util::prelude::*;
use weaver_winit::{Window, WindowResized};

use crate::{
    WgpuAdapter, WgpuDevice, WgpuInstance, WgpuQueue, resources::ActiveCommandEncoder,
    texture::texture_format::VIEW_FORMAT,
};

/// The window's wgpu surface and its current configuration.
pub struct WindowSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

impl WindowSurface {
    pub(crate) fn new(
        surface: wgpu::Surface<'static>,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let caps = surface.get_capabilities(adapter);
        ensure!(
            caps.formats.contains(&VIEW_FORMAT),
            "the window surface doesn't support {VIEW_FORMAT:?} (supported: {:?})",
            caps.formats
        );
        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes[0]
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
            format: VIEW_FORMAT,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Immediate,
            desired_maximum_frame_latency: 1,
            alpha_mode,
            view_formats: vec![],
        };
        surface.configure(device, &config);

        Ok(Self { surface, config })
    }

    pub fn width(&self) -> u32 {
        self.config.width
    }

    pub fn height(&self) -> u32 {
        self.config.height
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Reconfigures the surface for a new size. Zero sizes (a minimized window) are ignored, since
    /// a surface can't be configured with them; frames are skipped until the window is restored.
    /// Returns true if the size changed.
    fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        if width == 0 || height == 0 {
            return false;
        }
        if width == self.config.width && height == self.config.height {
            return false;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(device, &self.config);
        true
    }

    fn reconfigure(&self, device: &wgpu::Device) {
        self.surface.configure(device, &self.config);
    }
}

/// The swapchain texture being rendered this frame. Only exists between [`acquire_frame`] and
/// [`present_frame`], and only on frames where a texture could be acquired.
pub struct SurfaceFrame {
    texture: wgpu::SurfaceTexture,
    pub view: wgpu::TextureView,
}

impl SurfaceFrame {
    pub fn size(&self) -> wgpu::Extent3d {
        self.texture.texture.size()
    }
}

/// Command buffers recorded outside of the [`ActiveCommandEncoder`], submitted (after it) by
/// [`present_frame`].
#[derive(Default)]
pub struct CommandBuffers {
    buffers: Vec<wgpu::CommandBuffer>,
}

impl CommandBuffers {
    pub fn push(&mut self, buffer: wgpu::CommandBuffer) {
        self.buffers.push(buffer);
    }
}

/// Run condition for systems that draw into the current frame.
pub fn frame_in_flight() -> impl Condition {
    resource_exists::<SurfaceFrame>()
}

/// Creates the instance, adapter, device, queue, window surface and depth texture.
pub(crate) fn create_render_device(render_world: &mut World, window: &Window) -> Result<()> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        backend_options: Default::default(),
        flags: Default::default(),
        memory_budget_thresholds: Default::default(),
        display: Some(Box::new(window.display_handle.clone())),
    });

    let surface = create_raw_surface(&instance, window)?;

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .map_err(|e| anyhow!("no suitable GPU adapter found: {e}"))?;

    let required_limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::empty(),
        required_limits,
        label: None,
        memory_hints: wgpu::MemoryHints::Performance,
        experimental_features: wgpu::ExperimentalFeatures::default(),
        trace: wgpu::Trace::default(),
    }))
    .map_err(|e| anyhow!("failed to create the GPU device: {e}"))?;

    let size = window.inner_size();
    let surface = WindowSurface::new(surface, &adapter, &device, size.width, size.height)?;

    render_world.insert_resource(WgpuInstance { instance });
    render_world.insert_resource(WgpuAdapter { adapter });
    render_world.insert_resource(WgpuDevice { device });
    render_world.insert_resource(WgpuQueue { queue });
    render_world.insert_resource(surface);
    render_world.init_resource::<CommandBuffers>();

    Ok(())
}

fn create_raw_surface(
    instance: &wgpu::Instance,
    window: &Window,
) -> Result<wgpu::Surface<'static>> {
    // SAFETY: the window outlives the surface: both live in the render world, and the window
    // (an Arc'd winit window) is only dropped when the app shuts down
    let surface = unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::from_window(&**window)?)?
    };
    Ok(surface)
}

/// Reconfigures the surface after a window resize. Views pick up the new size in
/// [`crate::view::prepare_view_targets`].
pub fn resize_surface(
    events: EventRx<WindowResized>,
    device: Res<WgpuDevice>,
    mut surface: ResMut<WindowSurface>,
) {
    let Some(WindowResized { width, height }) = events.last() else {
        return;
    };
    if surface.resize(&device, width, height) {
        log::trace!("Resized surface to {width}x{height}");
    }
}

/// Acquires the next swapchain texture. On success, inserts [`SurfaceFrame`] and a fresh
/// [`ActiveCommandEncoder`]; otherwise the frame is skipped.
pub fn acquire_frame(
    commands: Commands,
    instance: Res<WgpuInstance>,
    device: Res<WgpuDevice>,
    window: Res<Window>,
    mut surface: ResMut<WindowSurface>,
) {
    let texture = match surface.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(texture) => texture,
        wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
            // still usable; reconfigure for the next frame
            surface.reconfigure(&device);
            texture
        }
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
            log::trace!("Skipping frame: surface texture unavailable");
            return;
        }
        wgpu::CurrentSurfaceTexture::Outdated => {
            log::debug!("Surface outdated; reconfiguring and skipping this frame");
            surface.reconfigure(&device);
            return;
        }
        wgpu::CurrentSurfaceTexture::Lost => {
            log::warn!("Surface lost; recreating it and skipping this frame");
            match create_raw_surface(&instance, &window) {
                Ok(raw) => {
                    surface.surface = raw;
                    surface.reconfigure(&device);
                }
                Err(e) => log::error!("Failed to recreate the surface: {e}"),
            }
            return;
        }
        other => {
            log::error!("Failed to acquire the next surface texture: {other:?}");
            return;
        }
    };

    let view = texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Encoder"),
    });

    commands.insert_resource(SurfaceFrame { texture, view });
    commands.insert_resource(ActiveCommandEncoder::new(encoder));
}

/// Submits the frame's command buffers and presents the swapchain texture.
pub fn present_frame(commands: Commands) {
    commands.add(|world| {
        let Some(frame) = world.remove_resource::<SurfaceFrame>() else {
            return;
        };
        let mut buffers = Vec::new();
        if let Some(encoder) = world.remove_resource::<ActiveCommandEncoder>() {
            buffers.push(encoder.finish());
        }
        if let Some(mut extra) = world.get_resource_mut::<CommandBuffers>() {
            buffers.append(&mut extra.buffers);
        }
        world.get_resource::<WgpuQueue>().unwrap().submit(buffers);
        frame.texture.present();
    });
}
