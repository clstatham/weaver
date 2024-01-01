//#import "src/renderer/pass/common.wgsl"

const NEAR_PLANE: f32 = 1.0;
const FAR_PLANE: f32 = 100.0;

@group(0) @binding(0) var shadow_cube_map: texture_cube<f32>;
@group(0) @binding(1) var shadow_cube_map_sampler: sampler;
@group(0) @binding(2) var color_in: texture_2d<f32>;
@group(0) @binding(3) var color_in_sampler: sampler;
@group(0) @binding(4) var<uniform> camera: CameraUniform;
@group(0) @binding(5) var<uniform> light: PointLight;
@group(0) @binding(6) var<uniform> model_transform: mat4x4<f32>;

fn linearize_depth(depth: f32) -> f32 {
    let z = depth * 2.0 - 1.0;
    return (2.0 * NEAR_PLANE * FAR_PLANE) / (FAR_PLANE + NEAR_PLANE - z * (FAR_PLANE - NEAR_PLANE));
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = camera.proj * camera.view * model_transform * vec4(input.position, 1.0);
    output.world_position = (model_transform * vec4(input.position, 1.0)).xyz;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let to_light = input.world_position - light.position.xyz;
    let current_depth = length(to_light);
    var shadow = 0.0;
    let radius = (1.0 + length(camera.camera_position.xyz - input.world_position) / FAR_PLANE) / 25.0;
    for (var i: i32 = -1; i <= 1; i += 1) {
        for (var j: i32 = -1; j <= 1; j += 1) {
            for (var k: i32 = -1; k <= 1; k += 1) {
                let offset = vec3<f32>(f32(i), f32(j), f32(k)) * radius;
                let sample_depth = textureSample(shadow_cube_map, shadow_cube_map_sampler, to_light + offset).r * FAR_PLANE;
                if current_depth - 0.05 > sample_depth {
                    shadow += 1.0;
                }
            }
        }
    }
    shadow /= 27.0;

    let visibility = 1.0 - shadow;

    let uv = input.clip_position.xy / vec2<f32>(textureDimensions(color_in));
    var color = textureSample(color_in, color_in_sampler, uv).rgb;

    color *= visibility;

    return vec4<f32>(color, 1.0);
}