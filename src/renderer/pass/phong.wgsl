struct LightUniform {
    position: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
}

@group(0) @binding(0) var color_buffer: texture_2d<f32>;
@group(0) @binding(1) var color_sampler: sampler;

@group(0) @binding(2) var normal_buffer: texture_2d<f32>;
@group(0) @binding(3) var normal_sampler: sampler;

@group(0) @binding(4) var depth_buffer: texture_depth_2d;
@group(0) @binding(5) var depth_sampler: sampler;

@group(0) @binding(6) var<uniform> light: LightUniform;
@group(0) @binding(7) var<uniform> inv_camera_projection: mat4x4<f32>;
@group(0) @binding(8) var<uniform> camera_pos: vec3<f32>;

fn screen_to_world(coord: vec2<f32>, depth_sample: f32) -> vec3<f32> {
    let posClip = vec4(coord.x * 2.0 - 1.0, (1.0 - coord.y) * 2.0 - 1.0, depth_sample, 1.0);
    let posWorldW = inv_camera_projection * posClip;
    let posWorld = posWorldW.xyz / posWorldW.www;
    return posWorld;
}
@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4<f32> {
    var pos = array(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0),
        vec2(-1.0, 1.0), vec2(1.0, -1.0), vec2(1.0, 1.0),
    );

    return vec4<f32>(pos[idx], 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) uv: vec4<f32>) -> @location(0) vec4<f32> {
    let depth = textureLoad(depth_buffer, vec2<i32>(floor(uv.xy)), 0);
    if depth >= 1.0 {
        discard;
    }
    let world_pos = screen_to_world(uv.xy, depth);
    let tex_color = textureLoad(color_buffer, vec2<i32>(floor(uv.xy)), 0).rgb;
    let normal = textureLoad(normal_buffer, vec2<i32>(floor(uv.xy)), 0).rgb;

    let light_dir = light.position - world_pos;
    let view_dir = camera_pos - world_pos;
    let half_dir = normalize(light_dir + view_dir);

    let diffuse_strength = max(dot(normal, half_dir), 0.0);
    let diffuse_color = light.color * diffuse_strength;

    let specular_strength = pow(diffuse_strength, 2.0);
    let specular_color = light.color * specular_strength;

    let ambient = 0.1;
    let ambient_color = light.color * ambient;

    let result = (ambient_color + specular_color + diffuse_color) * tex_color;

    return vec4(result, 1.0);
}