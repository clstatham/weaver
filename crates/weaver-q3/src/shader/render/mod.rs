use weaver_ecs::prelude::Resource;
use weaver_pbr::render::PbrLightingInformation;
use weaver_renderer::{
    bind_group::BindGroupLayoutCache,
    camera::CameraBindGroup,
    pipeline::RenderPipeline,
    prelude::{wgpu, RenderPipelineLayout},
    shader::Shader as RenderShader,
    texture::{texture_format, GpuTexture},
};
use weaver_util::prelude::FxHashMap;

use super::lexer::{BlendFunc, BlendFuncExplicitParam, Cull, LexedShader, LexedShaderStage};

pub mod extract;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderStagePipelineKey {
    pub blend_func: Option<BlendFunc>,
    pub cull: Cull,
}

impl ShaderStagePipelineKey {
    pub fn new(shader: &LexedShader, stage: &LexedShaderStage) -> Self {
        Self {
            blend_func: stage.blend_func().copied(),
            cull: shader.cull(),
        }
    }

    pub fn create_pipeline_layout(
        &self,
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
        shader_layout: &wgpu::BindGroupLayout,
    ) -> RenderPipelineLayout {
        let camera_layout = bind_group_layout_cache.get_or_create::<CameraBindGroup>(device);
        let lighting_layout =
            bind_group_layout_cache.get_or_create::<PbrLightingInformation>(device);

        RenderPipelineLayout::new(
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shader Stage Pipeline Layout"),
                bind_group_layouts: &[shader_layout, &camera_layout, &lighting_layout],
                push_constant_ranges: &[
                    // texture index
                    wgpu::PushConstantRange {
                        stages: wgpu::ShaderStages::FRAGMENT,
                        range: 0..std::mem::size_of::<u32>() as u32,
                    },
                ],
            }),
        )
    }

    pub fn create_pipeline(
        &self,
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
        shader_layout: &wgpu::BindGroupLayout,
    ) -> RenderPipeline {
        let layout = self.create_pipeline_layout(device, bind_group_layout_cache, shader_layout);
        let shader =
            RenderShader::new("assets/shaders/q3_shader_stage.wgsl").create_shader_module(device);
        RenderPipeline::new(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Q3 Shader Stage Pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 4 * (3 + 3 + 3 + 2) as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3, // Position
                            1 => Float32x3, // Normal
                            2 => Float32x3, // Tangent
                            3 => Float32x2, // TexCoord
                        ],
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: texture_format::HDR_FORMAT,
                        blend: self.blend_func.map(|func| wgpu::BlendState {
                            color: func.into(),
                            alpha: wgpu::BlendComponent::OVER,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: self.cull.into(),
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
            }),
        )
    }
}

pub struct KeyedShaderStage {
    pub key: ShaderStagePipelineKey,
    pub texture: GpuTexture,
}

#[derive(Resource, Default)]
pub struct KeyedShaderStagePipelineCache {
    pub map: FxHashMap<ShaderStagePipelineKey, RenderPipeline>,
}

impl KeyedShaderStagePipelineCache {
    pub fn get(&self, key: ShaderStagePipelineKey) -> Option<&RenderPipeline> {
        self.map.get(&key)
    }

    pub fn insert(&mut self, key: ShaderStagePipelineKey, pipeline: RenderPipeline) {
        self.map.insert(key, pipeline);
    }

    pub fn init(
        &mut self,
        key: ShaderStagePipelineKey,
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
        shader_layout: &wgpu::BindGroupLayout,
    ) {
        let pipeline = key.create_pipeline(device, bind_group_layout_cache, shader_layout);
        self.insert(key, pipeline);
    }

    pub fn get_or_init(
        &mut self,
        key: ShaderStagePipelineKey,
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
        shader_layout: &wgpu::BindGroupLayout,
    ) -> &RenderPipeline {
        self.map
            .entry(key)
            .or_insert_with(|| key.create_pipeline(device, bind_group_layout_cache, shader_layout))
    }
}
