struct Camera {
    view_proj: mat4x4<f32>,
    position: vec4<f32>,
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_position: vec3<f32>,
};

struct PointLight {
    position: vec4<f32>,
    color: vec4<f32>,
    intensity: f32,
};

@group(0) @binding(0) var<uniform> mesh_transform: mat4x4<f32>;
@group(0) @binding(1) var<uniform> camera: Camera;
@group(0) @binding(2) var tex: texture_2d<f32>;
@group(0) @binding(3) var tex_sampler: sampler;

@group(1) @binding(0) var<uniform> point_light: PointLight;

@vertex
fn vs_main(
    vtx: VertexInput,
) -> VertexOutput {
    var output: VertexOutput;
    output.world_normal = (mesh_transform * vec4<f32>(vtx.normal, 0.0)).xyz;
    output.world_position = (mesh_transform * vec4<f32>(vtx.position, 1.0)).xyz;
    output.uv = vtx.uv;
    output.clip_position = camera.view_proj * mesh_transform * vec4<f32>(vtx.position, 1.0);
    return output;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let object_color: vec4<f32> = textureSample(tex, tex_sampler, in.uv);

    // blinn-phong
    let ambient_strength = 0.01;
    let ambient = ambient_strength * point_light.color.xyz;
    let light_dir: vec3<f32> = normalize(point_light.position.xyz - in.world_position);
    let view_dir = normalize(camera.position.xyz - in.world_position);
    let half_dir = normalize(light_dir + view_dir);
    let light_intensity: f32 = point_light.intensity * pow(max(dot(in.world_normal, half_dir), 0.0), 32.0);
    let light_color: vec3<f32> = point_light.color.xyz * light_intensity + ambient;
    let result = vec4<f32>(object_color.rgb * light_color, 1.0);

    return result;
}