//! The per-view driver: the one system that records the frame's render passes.
//!
//! For every view it opens a single render pass on the view's [`ViewTarget`], clears it to the
//! [`ClearColor`], and draws the view's phases in a fixed order:
//! [`Background3d`], [`Opaque3d`], [`Overlay3d`]. Then it tonemaps the presenting view (the
//! [`PrimaryCamera`], or the first view if there is none) onto the swapchain texture.
//!
//! Plugins don't add render systems of their own; they queue phase items (see [`crate::phase`]).
//! Work that draws straight onto the swapchain texture after tonemapping (UI) can run in
//! [`RenderStage::Render`](crate::RenderStage::Render) `.after(render_views)`.

use weaver_core::color::Color;
use weaver_ecs::{
    entity::Entity,
    query::{Query, With},
    world::World,
};

use crate::{
    bind_group::BindGroup,
    camera::PrimaryCamera,
    clear_color::ClearColor,
    phase::{Background3d, DrawFunctions, Opaque3d, Overlay3d, RenderPhase},
    resources::ActiveCommandEncoder,
    surface::SurfaceFrame,
    tonemap::TonemapPipeline,
    tracked_pass::TrackedRenderPass,
    view::{ExtractedView, ViewTarget},
};

pub fn render_views(world: &World) {
    let Some(mut encoder) = world.get_resource_mut::<ActiveCommandEncoder>() else {
        return;
    };
    let clear_color = world
        .get_resource::<ClearColor>()
        .map_or(Color::BLACK, |clear_color| clear_color.0);

    let mut background = world
        .get_resource_mut::<DrawFunctions<Background3d>>()
        .unwrap();
    let mut opaque = world.get_resource_mut::<DrawFunctions<Opaque3d>>().unwrap();
    let mut overlay = world
        .get_resource_mut::<DrawFunctions<Overlay3d>>()
        .unwrap();

    #[allow(clippy::type_complexity)]
    let mut views = Query::<(
        Entity,
        &ViewTarget,
        &RenderPhase<Background3d>,
        &RenderPhase<Opaque3d>,
        &RenderPhase<Overlay3d>,
        With<ExtractedView>,
    )>::new(world);

    for (view, target, background_phase, opaque_phase, overlay_phase, _) in views.iter() {
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Main Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.hdr.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: clear_color.r as f64,
                        g: clear_color.g as f64,
                        b: clear_color.b as f64,
                        a: clear_color.a as f64,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &target.depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        let mut pass = TrackedRenderPass::new(pass);

        background_phase.render(world, &mut pass, view, &mut background);
        opaque_phase.render(world, &mut pass, view, &mut opaque);
        overlay_phase.render(world, &mut pass, view, &mut overlay);
    }
    drop(views);

    tonemap_presenting_view(world, &mut encoder);
}

/// Tonemaps the primary view (or the first view, if no view is primary) onto the swapchain
/// texture. This is the first pass to touch the swapchain texture each frame, so it clears it.
fn tonemap_presenting_view(world: &World, encoder: &mut wgpu::CommandEncoder) {
    let (Some(frame), Some(pipeline)) = (
        world.get_resource::<SurfaceFrame>(),
        world.get_resource::<TonemapPipeline>(),
    ) else {
        return;
    };

    let mut views = Query::<(
        &BindGroup<ViewTarget>,
        Option<&PrimaryCamera>,
        With<ExtractedView>,
    )>::new(world);
    let mut presenting = None;
    for (bind_group, primary, _) in views.iter() {
        let is_primary = primary.is_some();
        if presenting.is_none() || is_primary {
            presenting = Some(bind_group.bind_group().clone());
        }
        if is_primary {
            break;
        }
    }
    drop(views);

    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Tonemap Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &frame.view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        ..Default::default()
    });
    // with no views, the window is just cleared
    if let Some(bind_group) = presenting {
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, &*bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
