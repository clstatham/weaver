//#import "src/renderer/pass/common.wgsl"

const FAR_PLANE: f32 = 100.0;

@group(0) @binding(0) var<storage> model_transforms: array<mat4x4<f32>>;
@group(0) @binding(1) var<uniform> light: PointLight;
@group(0) @binding(2) var<uniform> light_view: mat4x4<f32>;

struct FragOut {
    @builtin(position) light_space_pos: vec4<f32>,
    @location(0) world_space_pos: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> FragOut {
    var output: FragOut;

    let model_transform = model_transforms[input.instance_index];

    let pos = model_transform * vec4<f32>(input.position, 1.0);

    let light_space_pos = light.proj_transform * light_view * pos;
    output.light_space_pos = light_space_pos;
    output.world_space_pos = pos;
    return output;
}

@fragment
fn fs_main(input: FragOut) -> @location(0) vec4<f32> {
    let light_distance = length(input.world_space_pos.xyz - light.position.xyz);
    let depth = light_distance / FAR_PLANE;
    return vec4<f32>(depth, 0.0, 0.0, 1.0);
}