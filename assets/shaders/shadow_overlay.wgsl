#define_import_path weaver::shadow_overlay
#import weaver::common::{CameraUniform, DirectionalLight};

@group(0) @binding(0) var shadow_map: texture_depth_2d;
@group(0) @binding(1) var shadow_map_sampler: sampler_comparison;
@group(0) @binding(2) var<uniform> camera: CameraUniform;
@group(0) @binding(3) var<uniform> light: DirectionalLight;
@group(0) @binding(4) var<storage> model_transforms: array<mat4x4<f32>>;

fn shadow_map_visiblity(pos: vec3<f32>) -> f32 {
    var visibility = 0.0;
    let one_over_shadow_tex_size = 1.0 / 1024.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let offset = vec2<f32>(vec2(x, y)) * one_over_shadow_tex_size;

            let uv = pos.xy + offset;
            visibility += textureSampleCompare(
                shadow_map, shadow_map_sampler,
                uv, pos.z - 0.001
            );
        }
    }
    visibility /= 9.0;

    return visibility;
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) instance_index: u32,
    @location(2) shadow_pos: vec3<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    let model_transform = model_transforms[input.instance_index];

    let pos_from_light = light.proj_transform * light.view_transform * model_transform * vec4(input.position, 1.0);

    output.shadow_pos = vec3(
        pos_from_light.xy * vec2(0.5, -0.5) + vec2(0.5),
        pos_from_light.z
    );

    output.clip_position = camera.proj * camera.view * model_transform * vec4(input.position, 1.0);

    output.instance_index = input.instance_index;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // get the shadow map visibility
    let visibility = shadow_map_visiblity(input.shadow_pos);

    return vec4<f32>(0.0, 0.0, 0.0, visibility);
}