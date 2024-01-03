struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct DoodadCamera {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> model_transform: mat4x4<f32>;
@group(0) @binding(1) var<uniform> camera: DoodadCamera;
@group(0) @binding(2) var<uniform> color: vec3<f32>;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = (model_transform * vec4<f32>(input.position, 1.0)).xyz;
    let normal_transform = mat3x3<f32>(
        model_transform[0].xyz,
        model_transform[1].xyz,
        model_transform[2].xyz,
    );
    output.normal = normal_transform * input.normal;
    output.clip_position = camera.projection * camera.view * vec4(output.position, 1.0);
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // simple phong lighting

    let light_direction = normalize(vec3<f32>(1.0, -1.0, 1.0));

    let light = max(dot(input.normal, light_direction), 0.0);
    let light_color = vec3<f32>(light);

    let ambient = vec3<f32>(0.5, 0.5, 0.5);

    var out_color = color.rgb * (light_color + ambient);
    out_color = min(out_color, vec3<f32>(1.0, 1.0, 1.0));

    return vec4<f32>(out_color, 1.0);
}
