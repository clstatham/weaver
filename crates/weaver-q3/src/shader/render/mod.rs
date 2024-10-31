use std::num::NonZeroU32;

use weaver_ecs::world::{ConstructFromWorld, World};
use weaver_pbr::render::PbrLightingInformation;
use weaver_renderer::{
    bind_group::BindGroupLayoutCache,
    camera::CameraBindGroup,
    pipeline::RenderPipeline,
    prelude::{wgpu, RenderPipelineLayout},
    shader::Shader as RenderShader,
    texture::texture_format,
    WgpuDevice,
};
use weaver_util::FxHashMap;

use super::lexer::{BlendFunc, BlendFuncExplicitParam, Cull};

pub const SHADER_TEXTURE_ARRAY_SIZE: u32 = 16;

#[allow(clippy::from_over_into)]
impl Into<Option<wgpu::Face>> for Cull {
    fn into(self) -> Option<wgpu::Face> {
        match self {
            Cull::Disable => None,
            Cull::Front => Some(wgpu::Face::Front),
            Cull::Back => Some(wgpu::Face::Back),
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<wgpu::BlendFactor> for BlendFuncExplicitParam {
    fn into(self) -> wgpu::BlendFactor {
        match self {
            BlendFuncExplicitParam::Zero => wgpu::BlendFactor::Zero,
            BlendFuncExplicitParam::One => wgpu::BlendFactor::One,
            BlendFuncExplicitParam::SrcColor => wgpu::BlendFactor::Src,
            BlendFuncExplicitParam::OneMinusSrcColor => wgpu::BlendFactor::OneMinusSrc,
            BlendFuncExplicitParam::DstColor => wgpu::BlendFactor::Dst,
            BlendFuncExplicitParam::OneMinusDstColor => wgpu::BlendFactor::OneMinusDst,
            BlendFuncExplicitParam::SrcAlpha => wgpu::BlendFactor::SrcAlpha,
            BlendFuncExplicitParam::OneMinusSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
            BlendFuncExplicitParam::DstAlpha => wgpu::BlendFactor::DstAlpha,
            BlendFuncExplicitParam::OneMinusDstAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<wgpu::BlendComponent> for BlendFunc {
    /// http://q3map2.robotrenegade.com/docs/shader_manual/stage-directives.html
    fn into(self) -> wgpu::BlendComponent {
        match self {
            BlendFunc::Add => wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            BlendFunc::Blend => wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            BlendFunc::Filter => wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Dst,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            },
            BlendFunc::Explicit(func) => {
                let src_factor: wgpu::BlendFactor = func.src.into();
                let dst_factor: wgpu::BlendFactor = func.dst.into();
                wgpu::BlendComponent {
                    src_factor,
                    dst_factor,
                    operation: wgpu::BlendOperation::Add,
                }
            }
        }
    }
}

pub struct ShaderBindGroupLayout {
    pub layout: wgpu::BindGroupLayout,
}

impl ConstructFromWorld for ShaderBindGroupLayout {
    fn from_world(world: &World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();

        let texture_binding = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: NonZeroU32::new(SHADER_TEXTURE_ARRAY_SIZE),
        };

        let sampler_binding = wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };

        let layout = wgpu::BindGroupLayoutDescriptor {
            entries: &[texture_binding, sampler_binding],
            label: Some("BSP Shader Bind Group Layout"),
        };

        let bind_group_layout = device.create_bind_group_layout(&layout);

        Self {
            layout: bind_group_layout,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderPipelineKey {
    pub blend_func: Option<BlendFunc>,
    pub cull: Cull,
}

#[derive(Default)]
pub struct ShaderPipelineCache {
    pub cache: FxHashMap<ShaderPipelineKey, ShaderPipeline>,
}

pub struct ShaderPipeline {
    pub layout: RenderPipelineLayout,
    pub pipeline: RenderPipeline,
}

impl ShaderPipeline {
    pub fn from_key(
        key: ShaderPipelineKey,
        device: &wgpu::Device,
        shader_layout: &ShaderBindGroupLayout,
        cache: &mut BindGroupLayoutCache,
    ) -> Self {
        let camera_layout = cache.get_or_create::<CameraBindGroup>(device);
        let lighting_layout = cache.get_or_create::<PbrLightingInformation>(device);

        let layout = RenderPipelineLayout::new(device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
                label: Some("Shader Stage Pipeline Layout"),
                bind_group_layouts: &[&shader_layout.layout, &camera_layout, &lighting_layout],
                push_constant_ranges: &[],
            },
        ));

        let shader =
            RenderShader::new("assets/shaders/q3_shader_stage.wgsl").create_shader_module(device);

        let pipeline = RenderPipeline::new(device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("Q3 Shader Stage Pipeline"),
                layout: Some(&layout),
                cache: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 4 * (3 + 3 + 3 + 2 + 1) as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3, // Position
                            1 => Float32x3, // Normal
                            2 => Float32x3, // Tangent
                            3 => Float32x2, // TexCoord
                            4 => Uint32, // Texture Index
                        ],
                    }],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: texture_format::HDR_FORMAT,
                        blend: key.blend_func.map(|func| wgpu::BlendState {
                            color: func.into(),
                            alpha: wgpu::BlendComponent::OVER,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: key.cull.into(),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: texture_format::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            },
        ));

        Self { layout, pipeline }
    }
}
