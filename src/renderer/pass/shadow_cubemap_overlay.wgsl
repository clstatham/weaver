//#import "src/renderer/pass/common.wgsl"

const FAR_PLANE: f32 = 100.0;

@group(0) @binding(0) var shadow_cube_map: texture_cube<f32>;
@group(0) @binding(1) var tex_sampler: sampler;
@group(0) @binding(2) var<uniform> camera: CameraUniform;
@group(0) @binding(3) var<uniform> light: PointLight;
@group(0) @binding(4) var<storage> model_transforms: array<mat4x4<f32>>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    let model_transform = model_transforms[input.instance_index];

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
                let sample_depth = textureSample(shadow_cube_map, tex_sampler, to_light + offset).r * FAR_PLANE;

                if current_depth - 0.05 > sample_depth {
                    shadow += 1.0;
                }
            }
        }
    }
    shadow /= 27.0;

    return vec4<f32>(0.0, 0.0, 0.0, shadow);
}