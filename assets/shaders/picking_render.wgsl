#define_import_path weaver::picking_render
#import weaver::common::{ModelTransform, CameraUniform, VertexInput};

// camera
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> entity: vec2<u32>;

@group(1) @binding(0) var<uniform> transform: ModelTransform;

// material information
@group(2) @binding(3) var          normal_tex: texture_2d<f32>;
@group(2) @binding(4) var          normal_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_binormal: vec3<f32>,
    @location(3) world_tangent: vec3<f32>,
    @location(5) uv: vec2<f32>,
}

@vertex
fn picking_vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // output
    output.clip_position = camera.proj * camera.view * transform.model * vec4<f32>(input.position, 1.0);
    output.world_position = (transform.model * vec4<f32>(input.position, 1.0)).xyz;
    output.world_normal = (transform.model * vec4<f32>(input.normal, 0.0)).xyz;
    output.world_tangent = (transform.model * vec4<f32>(input.tangent, 0.0)).xyz;
    output.world_binormal = normalize(cross(output.world_normal.xyz, output.world_tangent.xyz));
    output.uv = input.uv;

    return output;
}

struct FragmentOutput {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) entity: vec2<u32>,
}

@fragment
fn picking_fs_main(input: VertexOutput) -> FragmentOutput {
    var output: FragmentOutput;

    // normal
    let tex_normal = textureSample(normal_tex, normal_sampler, input.uv).xyz;
    let normal = normalize(input.world_normal * tex_normal.x + input.world_binormal * tex_normal.y + input.world_tangent * tex_normal.z);

    // output
    output.position = vec4<f32>(input.world_position, 1.0);
    output.normal = vec4<f32>(normal, 0.0);
    output.entity = entity;

    return output;
}