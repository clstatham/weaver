//#import "src/renderer/pass/common.wgsl"

// model information
@group(0) @binding(0) var<storage> model_transforms: array<mat4x4<f32>>;

// envorinment information
@group(1) @binding(0) var<uniform> camera: CameraUniform;

// material information
@group(2) @binding(0) var<uniform> material: MaterialUniform;
@group(2) @binding(1) var          diffuse_tex: texture_2d<f32>;
@group(2) @binding(2) var          normal_tex: texture_2d<f32>;
@group(2) @binding(3) var          roughness_tex: texture_2d<f32>;
@group(2) @binding(4) var          ao_tex: texture_2d<f32>;
@group(2) @binding(5) var          tex_sampler: sampler;


struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_binormal: vec3<f32>,
    @location(3) world_tangent: vec3<f32>,
    @location(4) world_bitangent: vec3<f32>,
    @location(5) uv: vec2<f32>,
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) position: vec4<f32>,
}


@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    let model_transform = model_transforms[input.instance_index];

    output.world_position = (model_transform * vec4<f32>(input.position, 1.0)).xyz;
    output.clip_position = camera.proj * camera.view * vec4<f32>(output.world_position, 1.0);
    output.uv = input.uv;

    // get just the rotation part of the model transform
    let normal_transform = mat3x3<f32>(
        model_transform[0].xyz,
        model_transform[1].xyz,
        model_transform[2].xyz
    );

    output.world_normal = normalize(normal_transform * input.normal);
    output.world_binormal = normalize(normal_transform * input.binormal);
    output.world_tangent = normalize(normal_transform * input.tangent);
    output.world_bitangent = normalize(normal_transform * input.bitangent);

    return output;
}


@fragment
fn fs_main(vertex: VertexOutput) -> FragmentOutput {
    var output: FragmentOutput;

    var tex_normal = textureSample(normal_tex, tex_sampler, material.texture_scale.x * vertex.uv).xyz;
    tex_normal = normalize(tex_normal * 2.0 - 1.0);

    // create TBN matrix
    let TBN = mat3x3<f32>(
        vertex.world_tangent,
        vertex.world_bitangent,
        vertex.world_normal
    );

    // transform normal from tangent space to world space
    let normal = normalize(TBN * tex_normal);
    output.normal = vec4<f32>(normal, 1.0);

    let tex_color = textureSample(diffuse_tex, tex_sampler, material.texture_scale.x * vertex.uv).xyz;
    let tex_ao = textureSample(ao_tex, tex_sampler, material.texture_scale.x * vertex.uv).r;

    output.color = vec4<f32>(tex_color * tex_ao, 1.0);

    output.position = vec4<f32>(vertex.world_position, 1.0);

    return output;
}

