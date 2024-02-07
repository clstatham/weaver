use std::sync::Arc;
use std::{fmt::Debug, path::Path};

use weaver_proc_macro::{BindableComponent, GpuComponent};

use fabricate::prelude::*;

use crate::prelude::Renderer;
use crate::renderer::internals::{
    BindGroupLayoutCache, BindableComponent, GpuComponent, GpuResourceManager, GpuResourceType,
    LazyBindGroup, LazyGpuHandle,
};

use super::color::Color;

pub trait TextureFormat: 'static {
    const FORMAT: wgpu::TextureFormat;
    const SAMPLE_TYPE: wgpu::TextureSampleType;

    fn texture(&self) -> &Texture;
}

macro_rules! texture_formats {
    ($($name:ident: $format:ident, $sample_type:expr;)*) => {
        $(
            #[derive(Clone, weaver_proc_macro::GpuComponent)]
            #[gpu(update = "update")]
            pub struct $name {
                #[gpu(component)]
                texture: Texture,
                bind_group: LazyBindGroup<Self>,
            }
            impl TextureFormat for $name {
                const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::$format;
                const SAMPLE_TYPE: wgpu::TextureSampleType = $sample_type;

                fn texture(&self) -> &Texture {
                    &self.texture
                }
            }

            impl Default for $name {
                fn default() -> Self {
                    Self {
                        texture: Texture::default_texture(),
                        bind_group: LazyBindGroup::default(),
                    }
                }
            }
        )*
    }
}

texture_formats! {
    WindowTexture: Bgra8UnormSrgb, wgpu::TextureSampleType::Float { filterable: true };
    SdrTexture: Rgba8UnormSrgb, wgpu::TextureSampleType::Float { filterable: true };
    HdrTexture: Rgba16Float, wgpu::TextureSampleType::Float { filterable: false };
    PositionMapTexture: Rgba32Float, wgpu::TextureSampleType::Float { filterable: false };
    NormalMapTexture: Rgba8Unorm, wgpu::TextureSampleType::Float { filterable: true };
    DepthTexture: Depth32Float, wgpu::TextureSampleType::Depth;
    HdrD2ArrayTexture: Rgba32Float, wgpu::TextureSampleType::Float { filterable: false };
    HdrCubeTexture: Rgba32Float, wgpu::TextureSampleType::Float { filterable: false };
    MonoTexture: R16Float, wgpu::TextureSampleType::Float { filterable: false };
    MonoCubeTexture: R16Float, wgpu::TextureSampleType::Float { filterable: false };
    DepthCubeTexture: Depth32Float, wgpu::TextureSampleType::Depth;
    MonoCubeArrayTexture: R16Float, wgpu::TextureSampleType::Float { filterable: false };
    DepthCubeArrayTexture: Depth32Float, wgpu::TextureSampleType::Depth;
}

macro_rules! texture_format_impls {
    ($dim:ident, $view_dim:ident, $layers:literal; $($name:ident),*) => {
        $(
            impl $name {
                pub fn new(
                    width: u32,
                    height: u32,
                    label: Option<&'static str>,
                ) -> Self {
                    Self {
                        texture: Texture::new_lazy(
                            width,
                            height,
                            label,
                            wgpu::TextureUsages::TEXTURE_BINDING
                                | wgpu::TextureUsages::COPY_DST
                                | wgpu::TextureUsages::COPY_SRC
                                | wgpu::TextureUsages::RENDER_ATTACHMENT,
                            Self::FORMAT,
                            wgpu::TextureDimension::$dim,
                            wgpu::TextureViewDimension::$view_dim,
                            1,
                        ),
                        bind_group: LazyBindGroup::default(),
                    }
                }


                pub fn from_texture(texture: Texture) -> Self {
                    Self {
                        texture,
                        bind_group: LazyBindGroup::default(),
                    }
                }

                pub fn handle(&self) -> &LazyGpuHandle {
                    self.texture.handle()
                }

                fn update(&self, world: &World) -> anyhow::Result<()> {
                    self.texture.update_resources(world)
                }

                pub fn resize(
                    &self,
                    renderer: &Renderer,
                    width: u32,
                    height: u32,
                ) {
                    self.texture.resize(
                        renderer,
                        width,
                        height,
                        wgpu::TextureViewDimension::$view_dim,
                        $layers,
                    );

                    self.bind_group.reset();
                }
            }

            impl BindableComponent for $name {
                fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some(concat!(stringify!($name), " bind group layout")),
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: Self::SAMPLE_TYPE,
                                view_dimension: wgpu::TextureViewDimension::$view_dim,
                                multisampled: false,
                            },
                            count: None,
                        }],
                    })
                }


                fn create_bind_group(
                    &self,
                    manager: &GpuResourceManager,
                    cache: &BindGroupLayoutCache,
                ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
                    let layout = cache.get_or_create::<Self>(
                        manager.device(),
                    );
                    let texture = self.texture.handle.lazy_init(manager)?;
                    let view = texture.get_texture().unwrap().create_view(&wgpu::TextureViewDescriptor {
                        label: Some(concat!(stringify!($name), " view")),
                        format: Some(Self::FORMAT),
                        dimension: Some(wgpu::TextureViewDimension::$view_dim),
                        array_layer_count: Some($layers),
                        ..Default::default()
                    });
                    let bind_group = manager.device().create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some(concat!(stringify!($name), " bind group")),
                        layout: &layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&view),
                        }],
                    });
                    Ok(Arc::new(bind_group))
                }

                fn bind_group(&self) -> Option<Arc<wgpu::BindGroup>> {
                    self.bind_group.bind_group().clone()
                }

                fn lazy_init_bind_group(
                    &self,
                    manager: &GpuResourceManager,
                    cache: &crate::renderer::internals::BindGroupLayoutCache,
                ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
                    if let Some(bind_group) = self.bind_group.bind_group() {
                        return Ok(bind_group);
                    }

                    let bind_group = self.bind_group.lazy_init_bind_group(manager, cache, self)?;
                    Ok(bind_group)
                }
            }
        )*
    };
}

texture_format_impls!(
    D2, D2, 1;
    WindowTexture,
    SdrTexture,
    HdrTexture,
    PositionMapTexture,
    NormalMapTexture,
    DepthTexture
);
texture_format_impls!(D2, Cube, 6; HdrCubeTexture, MonoCubeTexture, DepthCubeTexture);
texture_format_impls!(D2, D2Array, 6; HdrD2ArrayTexture, MonoTexture);
texture_format_impls!(D2, CubeArray, 6; MonoCubeArrayTexture, DepthCubeArrayTexture);

#[derive(Clone, GpuComponent)]
#[gpu(update = "update")]
pub struct Texture {
    pub(crate) handle: LazyGpuHandle,
}

impl Debug for Texture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Texture").finish()
    }
}

impl Texture {
    pub(crate) fn from_handle(handle: LazyGpuHandle) -> Self {
        Self { handle }
    }

    fn update(&self, _world: &World) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn handle(&self) -> &LazyGpuHandle {
        &self.handle
    }

    pub fn resize(
        &self,
        renderer: &Renderer,
        width: u32,
        height: u32,
        view_dimension: wgpu::TextureViewDimension,
        depth_or_array_layers: u32,
    ) {
        let current = self.handle.lazy_init(renderer.resource_manager()).unwrap();
        let current = current.get_texture().unwrap();
        if current.size().width == width && current.size().height == height {
            return;
        }

        let new = Texture::new_lazy(
            width,
            height,
            self.handle.label(),
            current.usage(),
            current.format(),
            current.dimension(),
            view_dimension,
            depth_or_array_layers,
        );

        let new_handle = new.handle.lazy_init(renderer.resource_manager()).unwrap();
        self.handle.reinit(new_handle);
    }

    pub fn load(
        path: impl AsRef<Path>,
        format: wgpu::TextureFormat,
        label: Option<&'static str>,
    ) -> Self {
        let path = path.as_ref();

        let image = image::open(path).unwrap().flipv().to_rgba8();
        let (width, height) = image.dimensions();

        Self::from_data_rgba8(width, height, &image, format, label)
    }

    pub fn from_data_rgba8(
        width: u32,
        height: u32,
        data: &[u8],
        format: wgpu::TextureFormat,
        label: Option<&'static str>,
    ) -> Self {
        let handle = LazyGpuHandle::new(
            GpuResourceType::Texture {
                width,
                height,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                dimension: wgpu::TextureDimension::D2,
                view_dimension: wgpu::TextureViewDimension::D2,
                depth_or_array_layers: 1,
            },
            label,
            Some(data.into()),
        );
        Self { handle }
    }

    pub fn from_data_r8g8b8(
        width: u32,
        height: u32,
        data: &[u8],
        format: wgpu::TextureFormat,
        label: Option<&'static str>,
    ) -> Self {
        // convert the data to RGBA
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for pixel in data.chunks(3) {
            rgba.extend_from_slice(pixel);
            rgba.push(255);
        }

        Self::from_data_rgba8(width, height, &rgba, format, label)
    }

    pub fn solid_color(
        color: Color,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        label: Option<&'static str>,
    ) -> Self {
        let (r, g, b, a) = color.rgba_int();
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..width * height {
            data.extend_from_slice(&[r, g, b, a]);
        }

        Self::from_data_rgba8(width, height, &data, format, label)
    }

    pub fn default_texture() -> Self {
        let width = 4;
        let height = 4;
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        for x in 0..width {
            for y in 0..height {
                // pink/white checkerboard
                let (r, g, b) = match (x < width / 2, y < height / 2) {
                    (true, true) | (false, false) => (255, 0, 255),
                    (true, false) | (false, true) => (255, 255, 255),
                };
                data.extend_from_slice(&[r, g, b, 255]);
            }
        }

        Self::from_data_rgba8(
            width,
            height,
            &data,
            SdrTexture::FORMAT,
            Some("Default Texture"),
        )
    }

    pub fn new_lazy(
        width: u32,
        height: u32,
        label: Option<&'static str>,
        usage: wgpu::TextureUsages,
        format: wgpu::TextureFormat,
        dimension: wgpu::TextureDimension,
        view_dimension: wgpu::TextureViewDimension,
        depth_or_array_layers: u32,
    ) -> Self {
        let handle = LazyGpuHandle::new(
            GpuResourceType::Texture {
                width,
                height,
                usage,
                format,
                dimension,
                view_dimension,
                depth_or_array_layers,
            },
            label,
            None,
        );
        Self { handle }
    }
}

#[derive(Clone, Atom, GpuComponent, BindableComponent)]
#[gpu(update = "update")]
pub struct Skybox {
    #[gpu(component)]
    #[texture(format = Rgba32Float, sample_type = float, view_dimension = Cube, layers = 6)]
    pub(crate) texture: HdrCubeTexture,

    #[gpu(component)]
    #[texture(format = Rgba32Float, sample_type = float, view_dimension = Cube, layers = 6)]
    pub(crate) irradiance: HdrCubeTexture,

    bind_group: LazyBindGroup<Self>,
}

impl Debug for Skybox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Skybox").finish()
    }
}

impl Skybox {
    pub(crate) fn new(texture: HdrCubeTexture, irradiance: HdrCubeTexture) -> Self {
        Self {
            texture,
            irradiance,
            bind_group: LazyBindGroup::default(),
        }
    }

    fn update(&self, world: &World) -> anyhow::Result<()> {
        self.texture.update(world)?;
        self.irradiance.update(world)?;
        Ok(())
    }
}
