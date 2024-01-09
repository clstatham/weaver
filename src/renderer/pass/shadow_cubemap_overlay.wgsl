//#import "src/renderer/pass/common.wgsl"

const FAR_PLANE: f32 = 100.0;

@group(0) @binding(0) var<storage> model_transforms: array<mat4x4<f32>>;

@group(1) @binding(0) var<uniform> camera: CameraUniform;

@group(2) @binding(0) var shadow_cube_maps: texture_cube_array<f32>;
@group(2) @binding(1) var tex_sampler: sampler;

@group(3) @binding(0) var<storage> lights: PointLights;


struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
}

@vertex
fn shadow_cubemap_overlay_vs(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    let model_transform = model_transforms[input.instance_index];

    output.clip_position = camera.proj * camera.view * model_transform * vec4(input.position, 1.0);
    output.world_position = (model_transform * vec4(input.position, 1.0)).xyz;
    return output;
}

@fragment
fn shadow_cubemap_overlay_fs(input: VertexOutput) -> @location(0) vec4<f32> {
    var shadow = 0.0;
    let radius = (1.0 + length(camera.camera_position.xyz - input.world_position) / FAR_PLANE) / 25.0;
    for (var li = 0u; li < lights.count; li += 1u) {
        let to_light = input.world_position - lights.lights[li].position.xyz;
        let current_depth = length(to_light);
        for (var i: i32 = -1; i <= 1; i += 1) {
            for (var j: i32 = -1; j <= 1; j += 1) {
                for (var k: i32 = -1; k <= 1; k += 1) {
                    let offset = vec3<f32>(f32(i), f32(j), f32(k)) * radius;
                    let sample_depth = textureSample(shadow_cube_maps, tex_sampler, to_light + offset, li).r * FAR_PLANE;

                    shadow += f32(current_depth - 0.5 > sample_depth);
                }
            }
        }
    }

    shadow /= f32(lights.count) * 27.0;

    return vec4<f32>(0.0, 0.0, 0.0, shadow);
}