//! Render and compute pipelines.
//!
//! Each pipeline lives in its own resource type (`PbrPipeline`, `SkyboxPipeline`, ...) that builds
//! itself in [`ConstructFromWorld::from_world`], usually with a [`RenderPipelineDescriptor`].
//! Register it with [`InitRenderResourcePlugin`], and draw systems take it as a plain `Res`.

use std::{marker::PhantomData, ops::Deref, sync::Arc};

use weaver_app::{App, plugin::Plugin};
use weaver_ecs::{
    commands::Commands,
    condition::{not, resource_exists},
    prelude::Component,
    system::IntoSystemConfig,
    world::ConstructFromWorld,
};
use weaver_util::prelude::*;

use crate::{
    RenderStage,
    bind_group::BindGroupLayout,
    texture::texture_format::{DEPTH_FORMAT, HDR_FORMAT},
};

#[derive(Clone)]
pub struct RenderPipeline {
    pipeline: Arc<wgpu::RenderPipeline>,
}

impl RenderPipeline {
    pub fn new(pipeline: wgpu::RenderPipeline) -> Self {
        Self {
            pipeline: Arc::new(pipeline),
        }
    }
}

impl Deref for RenderPipeline {
    type Target = wgpu::RenderPipeline;

    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

#[derive(Clone)]
pub struct ComputePipeline {
    pipeline: Arc<wgpu::ComputePipeline>,
}

impl ComputePipeline {
    pub fn new(pipeline: wgpu::ComputePipeline) -> Self {
        Self {
            pipeline: Arc::new(pipeline),
        }
    }
}

impl Deref for ComputePipeline {
    type Target = wgpu::ComputePipeline;

    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

/// Depth testing for a [`RenderPipelineDescriptor`].
#[derive(Debug, Clone, Copy)]
pub struct DepthTest {
    pub compare: wgpu::CompareFunction,
    pub write: bool,
}

impl Default for DepthTest {
    /// `LessEqual`, writing depth.
    fn default() -> Self {
        Self {
            compare: wgpu::CompareFunction::LessEqual,
            write: true,
        }
    }
}

/// A render pipeline description with the engine's usual defaults: `vs_main`/`fs_main` entry
/// points, one color target in the HDR format, alpha blending, no depth test, no culling, triangle
/// lists. Override what differs with the builder methods.
pub struct RenderPipelineDescriptor<'a> {
    pub label: &'static str,
    pub shader: &'a wgpu::ShaderModule,
    pub bind_group_layouts: Vec<BindGroupLayout>,
    pub vertex_entry: &'static str,
    pub fragment_entry: &'static str,
    pub vertex_buffers: Vec<wgpu::VertexBufferLayout<'static>>,
    pub color_format: wgpu::TextureFormat,
    pub blend: Option<wgpu::BlendState>,
    pub depth: Option<DepthTest>,
    pub cull_mode: Option<wgpu::Face>,
    pub topology: wgpu::PrimitiveTopology,
}

impl<'a> RenderPipelineDescriptor<'a> {
    pub fn new(label: &'static str, shader: &'a wgpu::ShaderModule) -> Self {
        Self {
            label,
            shader,
            bind_group_layouts: Vec::new(),
            vertex_entry: "vs_main",
            fragment_entry: "fs_main",
            vertex_buffers: Vec::new(),
            color_format: HDR_FORMAT,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            depth: None,
            cull_mode: None,
            topology: wgpu::PrimitiveTopology::TriangleList,
        }
    }

    /// Bind group layouts, in group order.
    pub fn bind_group_layouts(
        mut self,
        layouts: impl IntoIterator<Item = BindGroupLayout>,
    ) -> Self {
        self.bind_group_layouts = layouts.into_iter().collect();
        self
    }

    pub fn vertex_buffer(mut self, layout: wgpu::VertexBufferLayout<'static>) -> Self {
        self.vertex_buffers.push(layout);
        self
    }

    pub fn color_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.color_format = format;
        self
    }

    pub fn blend(mut self, blend: Option<wgpu::BlendState>) -> Self {
        self.blend = blend;
        self
    }

    /// Tests (and optionally writes) depth against a [`DEPTH_FORMAT`] attachment.
    pub fn depth(mut self, depth: DepthTest) -> Self {
        self.depth = Some(depth);
        self
    }

    pub fn cull_mode(mut self, face: Option<wgpu::Face>) -> Self {
        self.cull_mode = face;
        self
    }

    pub fn topology(mut self, topology: wgpu::PrimitiveTopology) -> Self {
        self.topology = topology;
        self
    }

    pub fn build(&self, device: &wgpu::Device) -> RenderPipeline {
        let bind_group_layouts = self
            .bind_group_layouts
            .iter()
            .map(|layout| Some(&**layout))
            .collect::<Vec<_>>();
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(self.label),
            bind_group_layouts: &bind_group_layouts,
            ..Default::default()
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(self.label),
            layout: Some(&layout),
            cache: None,
            vertex: wgpu::VertexState {
                module: self.shader,
                entry_point: Some(self.vertex_entry),
                buffers: &self.vertex_buffers,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: self.shader,
                entry_point: Some(self.fragment_entry),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.color_format,
                    blend: self.blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: self.topology,
                cull_mode: self.cull_mode,
                ..Default::default()
            },
            depth_stencil: self.depth.map(|depth| wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(depth.write),
                depth_compare: Some(depth.compare),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
        });

        RenderPipeline::new(pipeline)
    }
}

/// Creates a compute pipeline with the given bind group layouts.
pub fn create_compute_pipeline(
    device: &wgpu::Device,
    label: &'static str,
    shader: &wgpu::ShaderModule,
    entry_point: &'static str,
    bind_group_layouts: &[&BindGroupLayout],
) -> ComputePipeline {
    let bind_group_layouts = bind_group_layouts
        .iter()
        .map(|layout| Some(&***layout))
        .collect::<Vec<_>>();
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &bind_group_layouts,
        ..Default::default()
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        module: shader,
        entry_point: Some(entry_point),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    ComputePipeline::new(pipeline)
}

/// Creates render-world resource `T` (typically a pipeline) with `T::from_world` the first time
/// the render thread runs after it's missing.
///
/// This runs as a system gated on `T` not existing, rather than in `Plugin::finish`, so it works
/// for plugins added from inside other plugins too.
pub struct InitRenderResourcePlugin<T>(PhantomData<T>);

impl<T> Default for InitRenderResourcePlugin<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Component + ConstructFromWorld> Plugin for InitRenderResourcePlugin<T> {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_system(
            init_render_resource::<T>.run_if(not(resource_exists::<T>())),
            RenderStage::PrepareAssets,
        );
        Ok(())
    }
}

fn init_render_resource<T: Component + ConstructFromWorld>(commands: Commands) {
    commands.init_resource::<T>();
}
