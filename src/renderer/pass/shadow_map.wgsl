//#import "src/renderer/pass/common.wgsl"

@group(0) @binding(0) var<uniform> model_transform: mat4x4<f32>;
@group(0) @binding(1) var<uniform> directional_light: DirectionalLight;

@vertex
fn vs_main(
    vertex: VertexInput
) -> @builtin(position) vec4<f32> {
    return directional_light.proj_transform * directional_light.view_transform * model_transform * vec4<f32>(vertex.position, 1.0);
}
