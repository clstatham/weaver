struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @location(1) normal: vec4<f32>,
}

@group(0) @binding(0) var<uniform> model_matrix: mat4x4<f32>;
@group(0) @binding(1) var<uniform> view_matrix: mat4x4<f32>;
@group(0) @binding(2) var<uniform> projection_matrix: mat4x4<f32>;

@group(0) @binding(3) var tex: texture_2d<f32>;
@group(0) @binding(4) var tex_sampler: sampler;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.uv = input.uv;
    out.normal = (model_matrix * vec4<f32>(input.normal, 0.0)).xyz;
    out.clip_position = projection_matrix * view_matrix * model_matrix * vec4<f32>(input.position, 1.0);
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    out.color = textureSample(tex, tex_sampler, input.uv);
    out.normal = vec4(input.normal, 1.0);
    return out;
}