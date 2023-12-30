const PI: f32 = 3.1415926535897932384626433832795;
const MAX_LIGHTS: u32 = 16u;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) binormal: vec3<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
    @location(5) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_binormal: vec3<f32>,
    @location(3) world_tangent: vec3<f32>,
    @location(4) world_bitangent: vec3<f32>,
    @location(5) uv: vec2<f32>,
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

struct PointLight {
    position: vec4<f32>,
    color: vec4<f32>,
    intensity: f32,
    _pad: f32,
};

struct DirectionalLight {
    direction: vec4<f32>,
    color: vec4<f32>,
    intensity: f32,
    _pad: f32,
};

struct PointLights {
    lights: array<PointLight, MAX_LIGHTS>,
    count: u32,
};

struct DirectionalLights {
    lights: array<DirectionalLight, MAX_LIGHTS>,
    count: u32,
};


@group(0) @binding(0)  var<uniform> model_transform: mat4x4<f32>;
@group(0) @binding(1)  var<uniform> camera: CameraUniform;
@group(0) @binding(2)  var<uniform> material: MaterialUniform;
@group(0) @binding(3)  var          tex: texture_2d<f32>;
@group(0) @binding(4)  var          tex_sampler: sampler;
@group(0) @binding(5)  var          normal_tex: texture_2d<f32>;
@group(0) @binding(6)  var          normal_tex_sampler: sampler;
@group(0) @binding(7)  var          roughness_tex: texture_2d<f32>;
@group(0) @binding(8)  var          roughness_tex_sampler: sampler;
@group(0) @binding(9)  var<storage> point_lights: PointLights;
@group(0) @binding(10) var<storage> directional_lights: DirectionalLights;

fn fresnel_schlick(cos_theta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn dist_ggx(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;

    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;

    let denom = NdotH2 * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

fn geom_schlick_ggx(NdotV: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r * r) / 8.0;

    let denom = NdotV * (1.0 - k) + k;
    return NdotV / denom;
}

fn geom_smith(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, roughness: f32) -> f32 {
    let NdotV = max(dot(N, V), 0.0);
    let NdotL = max(dot(N, L), 0.0);
    let ggx2 = geom_schlick_ggx(NdotV, roughness);
    let ggx1 = geom_schlick_ggx(NdotL, roughness);
    return ggx1 * ggx2;
}

fn calculate_lighting(
    vertex: VertexOutput,
    albedo: vec3<f32>,
    tex_normal: vec3<f32>,
    light_direction: vec3<f32>,
    view_direction: vec3<f32>,
    light_color: vec3<f32>,
    light_intensity: f32
) -> vec3<f32> {
    let metallic = material.metallic.x;
    // roughness mapping
    let roughness = textureSample(roughness_tex, roughness_tex_sampler, vertex.uv).r * material.metallic.y;

    let N = normalize(tex_normal);
    let V = normalize(view_direction);
    let L = normalize(light_direction);
    let H = normalize(V + L);

    let radiance = light_color * light_intensity;

    // fresnel
    let F0 = mix(vec3(0.04), albedo, metallic);
    let F = fresnel_schlick(max(dot(H, V), 0.0), F0);

    // geometry
    let NDF = dist_ggx(N, H, roughness);
    let G = geom_smith(N, V, L, roughness);

    // cook-torrance brdf
    let numerator = NDF * G * F;
    let denominator = 4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0) + 0.0001;
    let specular = numerator / denominator;

    let kS = F;
    var kD = vec3(1.0) - kS;
    kD *= 1.0 - metallic;

    let NdotL = max(dot(N, L), 0.0);
    return (kD * albedo / PI + specular) * radiance * NdotL;
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.world_position = (model_transform * vec4<f32>(input.position, 1.0)).xyz;
    output.clip_position = camera.view_proj * vec4<f32>(output.world_position, 1.0);
    output.uv = input.uv;

    // get just the rotation part of the model transform
    let normal_transform = mat3x3<f32>(
        model_transform[0].xyz,
        model_transform[1].xyz,
        model_transform[2].xyz
    );

    output.world_normal = normalize(normal_transform * input.normal);
    output.world_binormal = normalize(normal_transform * input.binormal);
    output.world_tangent = normalize(normal_transform * input.tangent);
    output.world_bitangent = normalize(normal_transform * input.bitangent);

    return output;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    let texture_scale = material.texture_scale.x;

    let uv = vertex.uv * texture_scale;
    let tex_color = textureSample(tex, tex_sampler, uv).xyz;
    var tex_normal = textureSample(normal_tex, normal_tex_sampler, uv).xyz;
    tex_normal = normalize(tex_normal * 2.0 - 1.0);
    // var tex_normal = vertex.world_normal;

    // create TBN matrix
    let TBN = mat3x3<f32>(
        vertex.world_tangent,
        vertex.world_bitangent,
        vertex.world_normal
    );

    // transform normal from tangent space to world space
    tex_normal = normalize(TBN * tex_normal);

    let view_direction = normalize(camera.camera_position - vertex.world_position);

    var out_color = vec3<f32>(0.0, 0.0, 0.0);

    for (var i = 0u; i < point_lights.count; i = i + 1u) {
        let light = point_lights.lights[i];
        let light_pos = light.position.xyz;
        let light_direction = normalize(light_pos - vertex.world_position);
        out_color += calculate_lighting(vertex, material.base_color.xyz, tex_normal, light_direction, view_direction, light.color.xyz, light.intensity);
    }

    for (var i = 0u; i < directional_lights.count; i = i + 1u) {
        let light = directional_lights.lights[i];
        let light_direction = light.direction.xyz;
        out_color += calculate_lighting(vertex, material.base_color.xyz, tex_normal, light_direction, view_direction, light.color.xyz, light.intensity);
    }

    out_color *= tex_color;

    return vec4<f32>(out_color, 1.0);
}
