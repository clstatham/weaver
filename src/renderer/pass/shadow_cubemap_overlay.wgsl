//#import "src/renderer/pass/common.wgsl"

const FAR_PLANE: f32 = 100.0;

@group(0) @binding(0) var<storage> model_transforms: array<ModelTransform>;

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

    let model_transform = model_transforms[input.instance_index].model;

    output.clip_position = camera.proj * camera.view * model_transform * vec4(input.position, 1.0);
    output.world_position = (model_transform * vec4(input.position, 1.0)).xyz;
    return output;
}

@fragment
fn shadow_cubemap_overlay_fs(input: VertexOutput) -> @location(0) vec4<f32> {
    var shadow = 0.0;

    let radius = (1.0 + length(camera.camera_position.xyz - input.world_position) / FAR_PLANE) / 25.0;
    for (var li = 0u; li < lights.count; li += 1u) {
        let light = lights.lights[li];
        let to_light = input.world_position - light.position.xyz;
        let distance = length(to_light);
        let attenuation = light.intensity / (1.0 + distance * distance / (light.radius * light.radius));
        if attenuation < MIN_LIGHT_INTENSITY {
            // light is too far away, ignore it
            continue;
        }
        for (var i: i32 = -1; i <= 1; i += 1) {
            for (var j: i32 = -1; j <= 1; j += 1) {
                for (var k: i32 = -1; k <= 1; k += 1) {
                    let offset = vec3<f32>(f32(i), f32(j), f32(k)) * radius;
                    let sample_depth = textureSample(shadow_cube_maps, tex_sampler, to_light + offset, li).r * FAR_PLANE;

                    if distance - 0.5 > sample_depth {
                        shadow += f32(distance - 0.5 > sample_depth) / (27.0 * f32(lights.count));
                    }
                }
            }
        }
    }

    return vec4<f32>(0.0, 0.0, 0.0, shadow);
}