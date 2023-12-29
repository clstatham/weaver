const PI: f32 = 3.1415926535897932384626433832795;

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

fn frensel_factor(f0: vec3<f32>, product: f32) -> vec3<f32> {
    return mix(f0, vec3<f32>(1.0, 1.0, 1.0), pow(1.01 - product, 5.0));
}

fn phong_specular(v: vec3<f32>, l: vec3<f32>, n: vec3<f32>, specular: vec3<f32>, roughness: f32) -> vec3<f32> {
    let r = reflect(-l, n);
    let spec = max(dot(r, v), 0.0);
    let k = 1.999 / (roughness * roughness);
    return min(1.0, 3.0 * 0.0398 * k) * pow(spec, min(10000.0, k)) * specular;
}

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

    var out_color = tex_color;

    for (var i = 0u; i < arrayLength(&lights); i = i + 1u) {
        let light = lights[i];

        let tangent_matrix = transpose(mat3x3<f32>(vertex.world_tangent, vertex.world_bitangent, vertex.world_normal));
        let tangent_light_pos = tangent_matrix * light.position;

        let a = 20.0 / dot(tangent_light_pos - vertex.tangent_position, tangent_light_pos - vertex.tangent_position);

        let l = normalize(tangent_light_pos - vertex.tangent_position);
        let v = normalize(vertex.tangent_view_position - vertex.tangent_position);
        let h = normalize(l + v);
        let nn = normalize(tangent_normal);

        let binormal = cross(vertex.world_normal, vertex.world_tangent);
        let nb = normalize(binormal);
        let tbn = mat3x3<f32>(nb, cross(nn, nb), nn);

        let n = tbn * tangent_normal;
        let roughness = 0.5;
        let metallic = material.metallic.x;

        let f0 = vec3<f32>(0.04, 0.04, 0.04);

        let n_dot_l = max(dot(n, l), 0.0);
        let n_dot_v = max(dot(n, v), 0.001);
        let n_dot_h = max(dot(n, h), 0.001);
        let h_dot_v = max(dot(h, v), 0.001);
        let l_dot_v = max(dot(l, v), 0.001);

        let specular_frensel = frensel_factor(f0, n_dot_v);
        let spec = phong_specular(v, l, n, specular_frensel, roughness) * n_dot_l;

        let diffuse = (1.0 - specular_frensel) * (1.0 / PI) * n_dot_l;

        let reflected_light = spec * light.color * light.intensity;
        let diffuse_light = diffuse * light.color * light.intensity;

        out_color += reflected_light + (diffuse_light * mix(tex_color, vec3<f32>(0.0, 0.0, 0.0), metallic));
    }

    return vec4<f32>(out_color, 1.0);
}
