//! Views: the render-world side of cameras.
//!
//! Every active main-world [`Camera`] becomes an [`ExtractedView`] on its render-world mirror.
//! Each view owns a [`ViewTarget`] (an HDR color texture and a depth texture, sized like the
//! window), and the render phases it draws (see [`crate::phase`]).

use std::sync::Arc;

use weaver_app::{App, plugin::Plugin};
use weaver_core::geometry::{Aabb, Intersection};
use weaver_ecs::{
    commands::Commands,
    component::Res,
    entity::Entity,
    query::{Changed, Query, QueryableItem, With},
    system::IntoSystemConfig,
};
use weaver_util::prelude::*;

use crate::{
    RenderStage, WgpuDevice,
    bind_group::{AsBindGroup, BindGroupLayoutBuilder, BindingsBuilder, ComponentBindGroupPlugin},
    camera::Camera,
    extract::{ExtractComponent, ExtractComponentPlugin},
    surface::{WindowSurface, resize_surface},
    texture::{GpuTexture, texture_format},
};

/// A camera in the render world. Only active cameras are extracted.
#[derive(Clone, Copy, Debug)]
pub struct ExtractedView {
    pub camera: Camera,
}

impl ExtractedView {
    /// True if a mesh with local bounds `aabb` and model matrix `model` may be visible.
    pub fn may_see(&self, aabb: &Aabb, model: glam::Mat4) -> bool {
        let world_aabb = transform_aabb(aabb, model);
        self.camera
            .intersect_frustum_with_aabb(&world_aabb, true, true)
            != Intersection::Outside
    }
}

/// The axis-aligned bounds of `aabb` after transforming it by `matrix`.
fn transform_aabb(aabb: &Aabb, matrix: glam::Mat4) -> Aabb {
    let mut min = glam::Vec3::splat(f32::INFINITY);
    let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
    for corner in aabb.corners() {
        let corner = matrix.transform_point3(corner);
        min = min.min(corner);
        max = max.max(corner);
    }
    Aabb::new(min, max)
}

impl ExtractComponent for Camera {
    type Data = &'static Camera;
    type Filter = Changed<&'static Camera>;
    type Out = ExtractedView;

    fn extract(camera: QueryableItem<'_, Self::Data>) -> Option<Self::Out> {
        camera.active().then_some(ExtractedView { camera: *camera })
    }
}

/// Where a view renders: an HDR color texture (tonemapped onto the window at the end of the frame)
/// and a depth texture, both sized like the window.
pub struct ViewTarget {
    pub hdr: GpuTexture,
    pub depth: GpuTexture,
    sampler: Arc<wgpu::Sampler>,
}

impl ViewTarget {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let hdr = GpuTexture::new(
            device,
            Some("View HDR Target"),
            width,
            height,
            texture_format::HDR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth = GpuTexture::new(
            device,
            Some("View Depth Target"),
            width,
            height,
            texture_format::DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let sampler = Arc::new(device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("View HDR Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        }));
        Self {
            hdr,
            depth,
            sampler,
        }
    }

    pub fn size(&self) -> (u32, u32) {
        let size = self.hdr.texture.size();
        (size.width, size.height)
    }
}

/// The HDR target as tonemapping input.
impl AsBindGroup for ViewTarget {
    const LABEL: &'static str = "View Target";

    fn layout(layout: &mut BindGroupLayoutBuilder) {
        layout
            .texture(
                wgpu::ShaderStages::FRAGMENT,
                wgpu::TextureViewDimension::D2,
                wgpu::TextureSampleType::Float { filterable: false },
            )
            .sampler(
                wgpu::ShaderStages::FRAGMENT,
                wgpu::SamplerBindingType::NonFiltering,
            );
    }

    fn bindings<'a>(&'a self, bindings: &mut BindingsBuilder<'a>) {
        bindings.texture(&self.hdr).sampler(&self.sampler);
    }
}

/// Gives every view a [`ViewTarget`] the size of the window, recreating it after resizes.
pub fn prepare_view_targets(
    commands: Commands,
    device: Res<WgpuDevice>,
    surface: Res<WindowSurface>,
    mut views: Query<(Entity, With<ExtractedView>, Option<&ViewTarget>)>,
) {
    let size = (surface.width(), surface.height());
    for (view, _, target) in views.iter() {
        if target.is_none_or(|target| target.size() != size) {
            commands.insert_component(view, ViewTarget::new(&device, size.0, size.1));
        }
    }
}

pub struct ViewPlugin;

impl Plugin for ViewPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_plugin(ExtractComponentPlugin::<Camera>::default())?;
        render_app.add_plugin(ComponentBindGroupPlugin::<ViewTarget>::default())?;
        render_app.add_system(
            prepare_view_targets.after(resize_surface),
            RenderStage::Prepare,
        );
        Ok(())
    }
}
