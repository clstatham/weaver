#define_import_path weaver::shadow_map
#import weaver::common::{DirectionalLight, VertexInput};

@group(0) @binding(0) var<storage> model_transforms: array<mat4x4<f32>>;
@group(0) @binding(1) var<uniform> directional_light: DirectionalLight;

@vertex
fn vs_main(
    vertex: VertexInput
) -> @builtin(position) vec4<f32> {
    let model_transform = model_transforms[vertex.instance_index];
    return directional_light.proj_transform * directional_light.view_transform * model_transform * vec4<f32>(vertex.position, 1.0);
}
