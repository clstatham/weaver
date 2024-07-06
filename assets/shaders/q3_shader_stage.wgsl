#define_import_path weaver::q3_shader_stage
#import weaver::common::{ModelTransform, CameraUniform, MaterialUniform, VertexInput, MIN_LIGHT_INTENSITY, PI};


struct PointLight {
    position: vec4<f32>,
    color: vec4<f32>,
    intensity: f32,
    radius: f32,
};


struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_binormal: vec3<f32>,
    @location(3) world_tangent: vec3<f32>,
    @location(4) uv: vec2<f32>,
}

var<push_constant> tex_index: u32;

// material information
@group(0) @binding(0) var          tex: binding_array<texture_2d<f32>>;
@group(0) @binding(1) var          tex_sampler: sampler;

// camera information
@group(1) @binding(0) var<uniform> camera: CameraUniform;

// model information
// @group(2) @binding(0) var<storage> model_transforms: array<mat4x4<f32>>;

// lights information
@group(2) @binding(0) var<storage> point_lights: array<PointLight>;
@group(2) @binding(1) var          env_map_diffuse: texture_cube<f32>;
@group(2) @binding(2) var          env_map_specular: texture_cube<f32>;
@group(2) @binding(3) var          env_map_brdf: texture_2d<f32>;
@group(2) @binding(4) var          env_map_sampler: sampler;


@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // let model_transform = model_transforms[input.instance_index];

    let world_position = vec4<f32>(input.position, 1.0);

    output.world_position = world_position.xyz;
    output.clip_position = camera.proj * camera.view * world_position;
    output.uv = input.uv;

    var N = normalize(vec4<f32>(input.normal, 0.0).xyz);
    var T = normalize(vec4<f32>(input.tangent, 0.0).xyz);
    var B = normalize(cross(N, T));

    output.world_tangent = T;
    output.world_binormal = B;
    output.world_normal = N;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = input.world_position;
    let world_normal = input.world_normal;
    let world_tangent = input.world_tangent;
    let world_binormal = input.world_binormal;
    let tex_coord = input.uv;

    let tex_color = textureSample(tex[tex_index], tex_sampler, tex_coord);
    return vec4<f32>(tex_color.rgb, 1.0);
    // return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}