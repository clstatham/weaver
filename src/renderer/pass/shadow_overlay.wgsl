//#import "src/renderer/pass/common.wgsl"

@group(0) @binding(0) var shadow_map: texture_depth_2d;
@group(0) @binding(1) var shadow_map_sampler: sampler_comparison;
@group(0) @binding(2) var color_in: texture_2d<f32>;
@group(0) @binding(3) var color_in_sampler: sampler;
@group(0) @binding(4) var<uniform> camera: CameraUniform;
@group(0) @binding(5) var<uniform> light: DirectionalLight;
@group(0) @binding(6) var<uniform> model_transform: mat4x4<f32>;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let pos_from_light = light.proj_transform * light.view_transform * model_transform * vec4(input.position, 1.0);

    output.shadow_pos = vec3(
        pos_from_light.xy * vec2(0.5, -0.5) + vec2(0.5),
        pos_from_light.z
    );

    output.clip_position = camera.proj * camera.view * model_transform * vec4(input.position, 1.0);

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    var visibility = 0.0;
    let one_over_shadow_tex_size = 1.0 / 2048.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let offset = vec2<f32>(vec2(x, y)) * one_over_shadow_tex_size;

            let uv = input.shadow_pos.xy + offset;
            visibility += textureSampleCompare(
                shadow_map, shadow_map_sampler,
                uv, input.shadow_pos.z - 0.001
            );
        }
    }
    visibility /= 9.0;

    // get the fragment's UV coordinates in screen space
    var uv = input.clip_position.xy / vec2<f32>(textureDimensions(color_in));

    // get the color from the texture
    var color = textureSample(color_in, color_in_sampler, uv).rgb;

    // apply the shadow
    color *= visibility;

    return vec4(color, 1.0);
}