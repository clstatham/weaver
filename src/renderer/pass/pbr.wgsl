struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent_position: vec3<f32>,
    @location(5) tangent_view_position: vec3<f32>,
    @location(6) world_tangent: vec3<f32>,
    @location(7) world_bitangent: vec3<f32>,
}

struct CameraUniform {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_position: vec3<f32>,
};

struct MaterialUniform {
    base_color: vec4<f32>,
    metallic: vec4<f32>,
    texture_scale: vec4<f32>,
};

struct LightUniform {
    position: vec3<f32>,
    _pad1: u32,
    color: vec3<f32>,
    _pad2: u32,
    intensity: f32,
    _pad3: array<u32, 3>,
};

@group(0) @binding(0) var<uniform> model_transform: mat4x4<f32>;
@group(0) @binding(1) var<uniform> camera: CameraUniform;
@group(0) @binding(2) var<uniform> material: MaterialUniform;
@group(0) @binding(3) var          tex: texture_2d<f32>;
@group(0) @binding(4) var          tex_sampler: sampler;
@group(0) @binding(5) var          normal_tex: texture_2d<f32>;
@group(0) @binding(6) var          normal_tex_sampler: sampler;
@group(0) @binding(7) var<storage> lights: array<LightUniform>;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.world_position = (model_transform * vec4<f32>(input.position, 1.0)).xyz;
    output.world_normal = normalize((model_transform * vec4<f32>(input.normal, 0.0)).xyz);
    output.clip_position = camera.view_proj * vec4<f32>(output.world_position, 1.0);
    output.uv = input.uv;

    let world_normal = normalize((model_transform * vec4<f32>(input.normal, 0.0)).xyz);
    let world_tangent = normalize((model_transform * vec4<f32>(input.tangent, 0.0)).xyz);
    let world_bitangent = normalize((model_transform * vec4<f32>(input.bitangent, 0.0)).xyz);

    let tangent_matrix = transpose(mat3x3<f32>(world_tangent, world_bitangent, world_normal));

    output.world_tangent = world_tangent;
    output.world_bitangent = world_bitangent;
    output.tangent_position = tangent_matrix * output.world_position;
    output.tangent_view_position = tangent_matrix * camera.camera_position;

    return output;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    let texture_scale = material.texture_scale.x;
    let metallic = material.metallic.x;

    let uv = vertex.uv * texture_scale;
    let tex_color = textureSample(tex, tex_sampler, uv).xyz;
    let tex_normal = textureSample(normal_tex, normal_tex_sampler, uv).xyz;

    let tangent_normal = normalize(tex_normal * 2.0 - 1.0);

    var out_color = vec3<f32>(0.0, 0.0, 0.0);

    for (var i = 0u; i < arrayLength(&lights); i = i + 1u) {
        let light = lights[i];

        let tangent_matrix = transpose(mat3x3<f32>(vertex.world_tangent, vertex.world_bitangent, vertex.world_normal));
        let tangent_light_pos = tangent_matrix * light.position;

        let light_dir = normalize(tangent_light_pos - vertex.tangent_position);
        let view_dir = normalize(vertex.tangent_view_position - vertex.tangent_position);
        let half_dir = normalize(light_dir + view_dir);

        let diffuse = max(dot(tangent_normal, light_dir), 0.0);
        let specular = pow(max(dot(tangent_normal, half_dir), 0.0), metallic);
        out_color += light.color * (diffuse + specular) * light.intensity;
    }

    out_color += 0.1; // ambient

    out_color *= tex_color;

    return vec4<f32>(out_color, 1.0);
}
