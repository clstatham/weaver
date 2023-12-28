struct Camera {
    view_proj: mat4x4<f32>,
    position: vec4<f32>,
};

@group(0) @binding(0) var<uniform> mesh_transform: mat4x4<f32>;
@group(0) @binding(1) var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(
    vtx: VertexInput,
) -> VertexOutput {
    var output: VertexOutput;
    output.color = vtx.color;
    output.clip_position = camera.view_proj * mesh_transform * vec4<f32>(vtx.position, 1.0);
    return output;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}