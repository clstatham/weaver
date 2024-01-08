//#import "src/renderer/pass/common.wgsl"

const FAR_PLANE: f32 = 100.0;

@group(0) @binding(0) var<storage> model_transforms: array<mat4x4<f32>>;
@group(1) @binding(0) var<uniform> light: PointLight;
@group(2) @binding(0) var<storage> light_views: array<mat4x4<f32>, 6>;

struct VertexOutput {
    @builtin(position) light_space_pos: vec4<f32>,
    @location(0) world_space_pos: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput, @builtin(view_index) view_index: i32) -> VertexOutput {
    var output: VertexOutput;

    let model_transform = model_transforms[input.instance_index];
    let light_view = light_views[view_index];

    let pos = model_transform * vec4<f32>(input.position, 1.0);

    let light_space_pos = light.proj_transform * light_view * pos;
    output.light_space_pos = light_space_pos;
    output.world_space_pos = pos;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) f32 {
    let light_distance = length(input.world_space_pos.xyz - light.position.xyz);
    let depth = light_distance / FAR_PLANE;
    return depth;
}