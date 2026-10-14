//! The skybox: an HDR image converted to a cubemap and drawn behind the scene.
//!
//! [`Skybox`] is the main-world resource naming the files. In the render world [`GpuSkybox`] turns
//! the equirectangular `.hdr` into a cubemap with a compute pass, and [`SkyboxPipeline`] draws it
//! as a fullscreen triangle in the [`Background3d`] phase. The lighting maps derived from the same
//! sky live in [`irradiance`].

use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
};

use image::{ImageDecoder, codecs::hdr::HdrDecoder};
use weaver_app::plugin::Plugin;
use weaver_ecs::{
    component::Res,
    condition::{not, resource_exists},
    entity::Entity,
    prelude::{Commands, World},
    query::Query,
    system::{IntoSystemConfig, SystemParamItem},
    world::ConstructFromWorld,
};
use weaver_renderer::{
    RenderStage, WgpuDevice, WgpuQueue,
    bind_group::{
        AsBindGroup, BindGroup, BindGroupLayoutBuilder, BindGroupLayouts, BindingsBuilder,
        ResourceBindGroupPlugin,
    },
    camera::CameraBindGroup,
    extract::{ExtractResource, ExtractResourcePlugin},
    phase::{AddDrawFunction, Background3d, DrawFunction, DrawFunctions, RenderPhase},
    pipeline::{
        DepthTest, InitRenderResourcePlugin, RenderPipeline, RenderPipelineDescriptor,
        create_compute_pipeline,
    },
    prelude::wgpu,
    shader::ShaderLibrary,
    texture::{GpuTexture, texture_format},
    tracked_pass::TrackedRenderPass,
};
use weaver_util::prelude::*;

pub mod irradiance;

/// Face size in pixels of the background cubemap converted from the equirectangular image.
pub const SKYBOX_CUBEMAP_SIZE: u32 = 1024;

/// The sky: an equirectangular HDR image and the files for lighting the scene with it.
///
/// Insert it as a resource in the main world. The background cubemap is converted from `path` on
/// the GPU; the other three files are pre-generated (see `assets/skyboxes/`) and are what
/// actually light the meshes.
///
/// # Examples
///
/// ```
/// use weaver_pbr::skybox::Skybox;
///
/// let sky = Skybox::new("assets/skyboxes/meadow_2k.hdr");
/// assert!(sky.diffuse_path.ends_with("meadow_2k_diffuse.ktx2"));
/// ```
#[derive(Clone)]
pub struct Skybox {
    /// The equirectangular Radiance `.hdr` image.
    pub path: PathBuf,
    /// Diffuse irradiance cubemap (`.ktx2`).
    pub diffuse_path: PathBuf,
    /// Prefiltered specular cubemap with mips (`.ktx2`).
    pub specular_path: PathBuf,
    /// BRDF lookup table (`.png`).
    pub brdf_lut_path: PathBuf,
}

impl Skybox {
    /// Creates a skybox from an `.hdr` path, finding the other files next to it.
    ///
    /// For `sky.hdr` those are `sky_diffuse.ktx2`, `sky_specular.ktx2` and `sky_LUT.png` in the
    /// same directory. Nothing is read until the render world picks the resource up.
    ///
    /// # Panics
    ///
    /// Panics if `path` has no file name or it isn't valid UTF-8.
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let file_stem = path.file_stem().unwrap().to_str().unwrap().to_owned();
        let diffuse_path = path
            .with_file_name(file_stem.clone() + "_diffuse")
            .with_extension("ktx2");
        let specular_path = path
            .with_file_name(file_stem.clone() + "_specular")
            .with_extension("ktx2");
        let brdf_lut_path = path
            .with_file_name(file_stem.clone() + "_LUT")
            .with_extension("png");

        Self {
            path,
            diffuse_path,
            specular_path,
            brdf_lut_path,
        }
    }
}

impl ExtractResource for Skybox {
    type Source = Skybox;

    fn extract_render_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

pub(crate) struct GpuSkyboxSrc {
    src_texture: GpuTexture,
}

impl AsBindGroup for GpuSkyboxSrc {
    const LABEL: &'static str = "Skybox Loader Source";

    fn layout(layout: &mut BindGroupLayoutBuilder) {
        layout.texture(
            wgpu::ShaderStages::COMPUTE,
            wgpu::TextureViewDimension::D2,
            wgpu::TextureSampleType::Float { filterable: false },
        );
    }

    fn bindings<'a>(&'a self, bindings: &mut BindingsBuilder<'a>) {
        bindings.texture(&self.src_texture);
    }
}

pub(crate) struct GpuSkyboxDst {
    dst_texture: GpuTexture,
}

/// The skybox background as a cubemap in the render world, bound as group 0 of
/// [`SkyboxPipeline`].
#[derive(Clone)]
pub struct GpuSkybox {
    #[allow(unused)]
    pub(crate) texture: GpuTexture,
    pub(crate) cube_view: Arc<wgpu::TextureView>,
    pub(crate) sampler: Arc<wgpu::Sampler>,
}

impl ConstructFromWorld for GpuSkybox {
    fn from_world(world: &World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();
        let queue = world.get_resource::<WgpuQueue>().unwrap();
        let skybox = world.get_resource::<Skybox>().unwrap();
        let layouts = world.get_resource::<BindGroupLayouts>().unwrap();
        let shaders = world.get_resource::<ShaderLibrary>().unwrap();

        Self::new(&skybox, &device, &queue, &layouts, &shaders)
    }
}

impl GpuSkybox {
    /// Loads the skybox's equirectangular HDR image and converts it to a cube map on the GPU.
    ///
    /// Runs synchronously: decodes the image, uploads it and submits the conversion compute pass
    /// to `queue` before returning.
    ///
    /// # Panics
    ///
    /// Panics if `skybox.path` can't be opened or isn't a valid Radiance HDR file.
    pub fn new(
        skybox: &Skybox,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layouts: &BindGroupLayouts,
        shaders: &ShaderLibrary,
    ) -> Self {
        let file = File::open(&skybox.path).unwrap();

        let hdr_decoder = HdrDecoder::new(file).unwrap();
        let meta = hdr_decoder.metadata();
        let mut pixels_buf = vec![0; hdr_decoder.total_bytes() as usize];
        hdr_decoder.read_image(&mut pixels_buf).unwrap();
        let mut pixels = vec![[0.0, 0.0, 0.0, 0.0]; (meta.width * meta.height) as usize];
        for (i, chunk) in pixels_buf
            .as_chunks::<{ 3 * size_of::<f32>() }>()
            .0
            .iter()
            .enumerate()
        {
            let rgb = bytemuck::cast_slice::<u8, f32>(chunk);
            pixels[i] = [rgb[0], rgb[1], rgb[2], 1.0];
        }

        let src = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Skybox Source Texture"),
            size: wgpu::Extent3d {
                width: meta.width,
                height: meta.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format::HDR_CUBE_FORMAT,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let src_view = src.create_view(&wgpu::TextureViewDescriptor::default());
        queue.write_texture(
            src.as_image_copy(),
            bytemuck::cast_slice(&pixels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(meta.width * std::mem::size_of::<[f32; 4]>() as u32),
                rows_per_image: Some(meta.height),
            },
            wgpu::Extent3d {
                width: meta.width,
                height: meta.height,
                depth_or_array_layers: 1,
            },
        );

        let dst = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Skybox Destination Texture"),
            size: wgpu::Extent3d {
                width: SKYBOX_CUBEMAP_SIZE,
                height: SKYBOX_CUBEMAP_SIZE,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format::HDR_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let dst_view = dst.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(6),
            ..Default::default()
        });

        let src = GpuSkyboxSrc {
            src_texture: GpuTexture::from_parts(src, src_view),
        };
        let dst = GpuSkyboxDst {
            dst_texture: GpuTexture::from_parts(dst, dst_view),
        };

        let src_layout = layouts.get::<GpuSkyboxSrc>(device);
        let dst_layout = layouts.get::<GpuSkyboxDst>(device);
        let src_bind_group = BindGroup::new(device, &src_layout, &src);
        let dst_bind_group = BindGroup::new(device, &dst_layout, &dst);

        // only used once, so it isn't kept around
        let shader = shaders.load(device, "skybox_loader.wesl");
        let pipeline = create_compute_pipeline(
            device,
            "Skybox Loader Pipeline",
            &shader,
            "load",
            &[&src_layout, &dst_layout],
        );

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Skybox Load Encoder"),
        });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Skybox Load Pass"),
                timestamp_writes: None,
            });
            let num_workgroups = SKYBOX_CUBEMAP_SIZE.div_ceil(16);
            cpass.set_pipeline(&pipeline);
            cpass.set_bind_group(0, &*src_bind_group, &[]);
            cpass.set_bind_group(1, &*dst_bind_group, &[]);
            cpass.dispatch_workgroups(num_workgroups, num_workgroups, 6);
        }
        queue.submit(Some(encoder.finish()));

        let GpuSkyboxDst { dst_texture } = dst;

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Skybox Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        let cube_view = dst_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                array_layer_count: Some(6),
                ..Default::default()
            });

        Self {
            texture: dst_texture,
            cube_view: Arc::new(cube_view),
            sampler: Arc::new(sampler),
        }
    }
}

impl AsBindGroup for GpuSkyboxDst {
    const LABEL: &'static str = "Skybox Loader Destination";

    fn layout(layout: &mut BindGroupLayoutBuilder) {
        layout.storage_texture(
            wgpu::ShaderStages::COMPUTE,
            wgpu::StorageTextureAccess::WriteOnly,
            texture_format::HDR_FORMAT,
            wgpu::TextureViewDimension::D2Array,
        );
    }

    fn bindings<'a>(&'a self, bindings: &mut BindingsBuilder<'a>) {
        bindings.storage_texture(&self.dst_texture);
    }
}

impl AsBindGroup for GpuSkybox {
    const LABEL: &'static str = "Skybox";

    fn layout(layout: &mut BindGroupLayoutBuilder) {
        let fragment = wgpu::ShaderStages::FRAGMENT;
        layout.texture_cube(fragment).filtering_sampler(fragment);
    }

    fn bindings<'a>(&'a self, bindings: &mut BindingsBuilder<'a>) {
        bindings
            .texture_view(&self.cube_view)
            .sampler(&self.sampler);
    }
}

/// Draws the [`GpuSkybox`] as a fullscreen triangle (`sky.wesl`), without blending.
pub struct SkyboxPipeline {
    /// The compiled pipeline.
    pub pipeline: RenderPipeline,
}

impl ConstructFromWorld for SkyboxPipeline {
    fn from_world(world: &World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();
        let layouts = world.get_resource::<BindGroupLayouts>().unwrap();
        let shaders = world.get_resource::<ShaderLibrary>().unwrap();
        let shader = shaders.load(&device, "sky.wesl");

        let pipeline = RenderPipelineDescriptor::new("Skybox Pipeline", &shader)
            .bind_group_layouts([
                layouts.get::<GpuSkybox>(&device),
                layouts.get::<CameraBindGroup>(&device),
            ])
            .blend(None)
            .depth(DepthTest::default())
            .build(&device);

        Self { pipeline }
    }
}

/// Puts the skybox in every view's background.
fn queue_skybox(
    draw_functions: Res<DrawFunctions<Background3d>>,
    mut views: Query<(Entity, &mut RenderPhase<Background3d>)>,
) {
    let draw_function = draw_functions.id::<DrawSkybox>();
    for (view, mut phase) in views.iter() {
        phase.add(Background3d {
            view,
            draw_function,
        });
    }
}

/// Draws the skybox behind everything with a fullscreen triangle.
struct DrawSkybox;

impl DrawFunction<Background3d> for DrawSkybox {
    type Param = (
        Res<SkyboxPipeline>,
        Res<BindGroup<GpuSkybox>>,
        Query<&'static BindGroup<CameraBindGroup>>,
    );

    fn draw(
        (pipeline, skybox, cameras): &mut SystemParamItem<Self::Param>,
        pass: &mut TrackedRenderPass,
        view: Entity,
        _items: &[Background3d],
    ) {
        let Some(camera) = cameras.get(view) else {
            return;
        };
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, skybox);
        pass.set_bind_group(1, &camera);
        pass.draw(0..3, 0..1);
    }
}

/// Extracts the [`Skybox`] and builds the [`GpuSkybox`] from it.
pub struct SkyboxPlugin;

impl Plugin for SkyboxPlugin {
    fn build(&self, render_app: &mut weaver_app::App) -> Result<()> {
        render_app.add_plugin(ExtractResourcePlugin::<Skybox>::default())?;
        render_app.add_plugin(ResourceBindGroupPlugin::<GpuSkybox>::default())?;

        render_app.add_system(
            init_gpu_skybox
                .run_if(resource_exists::<Skybox>())
                .run_if(not(resource_exists::<GpuSkybox>())),
            RenderStage::Prepare,
        );

        Ok(())
    }
}

// FIXME: built once, so changing the `Skybox` resource later doesn't update the background
fn init_gpu_skybox(commands: Commands) {
    commands.init_resource::<GpuSkybox>();
}

/// Draws the skybox in the [`Background3d`] phase of every view.
pub struct SkyboxRenderablePlugin;

impl Plugin for SkyboxRenderablePlugin {
    fn build(&self, render_app: &mut weaver_app::App) -> Result<()> {
        render_app.add_plugin(InitRenderResourcePlugin::<SkyboxPipeline>::default())?;

        render_app.add_draw_function::<Background3d, DrawSkybox>();
        render_app.add_system(
            queue_skybox.run_if(resource_exists::<BindGroup<GpuSkybox>>()),
            RenderStage::Queue,
        );

        Ok(())
    }
}
