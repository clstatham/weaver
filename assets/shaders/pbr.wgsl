#define_import_path weaver::pbr
#import weaver::common::{ModelTransform, CameraUniform, MaterialUniform, VertexInput, MIN_LIGHT_INTENSITY, PI};

struct PointLight {
    position: vec4<f32>,
    color: vec4<f32>,
    intensity: f32,
    radius: f32,
};

// material information
@group(0) @binding(0) var<uniform> material: MaterialUniform;
@group(0) @binding(1) var          diffuse_tex: texture_2d<f32>;
@group(0) @binding(2) var          diffuse_sampler: sampler;
@group(0) @binding(3) var          normal_tex: texture_2d<f32>;
@group(0) @binding(4) var          normal_sampler: sampler;
@group(0) @binding(5) var          roughness_tex: texture_2d<f32>;
@group(0) @binding(6) var          roughness_sampler: sampler;
@group(0) @binding(7) var          ao_tex: texture_2d<f32>;
@group(0) @binding(8) var          ao_sampler: sampler;

// camera information
@group(1) @binding(0) var<uniform> camera: CameraUniform;

// model information
@group(2) @binding(0) var<uniform> model_transform: mat4x4<f32>;

// lights information
@group(3) @binding(0) var<storage> point_lights: array<PointLight>;
@group(3) @binding(1) var          env_map_diffuse: texture_cube<f32>;
@group(3) @binding(2) var          env_map_specular: texture_cube<f32>;
@group(3) @binding(3) var          env_map_brdf: texture_2d<f32>;
@group(3) @binding(4) var          env_map_sampler: sampler;

fn saturate(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

fn fresnel_schlick(f0: vec3<f32>, HdV: f32, roughness: f32) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(saturate(1.0 - HdV), 5.0);
}

fn d_ggx(NdH: f32, roughness: f32) -> f32 {
    let m = roughness * roughness;
    let m2 = m * m;
    let NdH2 = NdH * NdH;
    let d = (NdH2 * (m2 - 1.0) + 1.0);
    return m2 / (PI * d * d);
}

fn g_schlick_ggx(roughness: f32, NdV: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;

    return NdV / (NdV * (1.0 - k) + k);
}

fn g_smith(roughness: f32, NdV: f32, NdL: f32) -> f32 {
    let ggx1 = g_schlick_ggx(roughness, NdV);
    let ggx2 = g_schlick_ggx(roughness, NdL);

    return ggx1 * ggx2;
}

fn cooktorrance_brdf(NdL: f32, NdV: f32, NdH: f32, F: vec3<f32>, roughness: f32) -> vec3<f32> {
    let NDF = d_ggx(NdH, roughness);
    let G = g_smith(roughness, NdV, NdL);

    let num = NDF * G * F;
    let denom = 4.0 * NdV * NdL + 0.0001;
    return num / denom;
}

fn importance_sample_ggx(uv: vec2<f32>, N: vec3<f32>, roughness: f32) -> vec3<f32> {
    let a = roughness * roughness;

    let phi = 2.0 * PI * uv.x;
    let cos_theta = sqrt((1.0 - uv.y) / (1.0 + (a * a - 1.0) * uv.y));
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);

    let H = vec3<f32>(
        sin_theta * cos(phi),
        sin_theta * sin(phi),
        cos_theta,
    );

    var up: vec3<f32>;
    if abs(N.z) < 0.999 {
        up = vec3<f32>(0.0, 0.0, 1.0);
    } else {
        up = vec3<f32>(1.0, 0.0, 0.0);
    }
    let tangent = normalize(cross(up, N));
    let bitangent = cross(N, tangent);

    let sample = tangent * H.x + bitangent * H.y + N * H.z;
    return normalize(sample);
}

fn calculate_lighting(
    albedo: vec3<f32>,
    roughness: f32,
    metallic: f32,
    N: vec3<f32>,
    L: vec3<f32>,
    V: vec3<f32>,
    light_color: vec3<f32>,
    attenuation: f32,
) -> vec3<f32> {
    let H = normalize(V + L);

    let NdL = max(dot(N, L), 0.0);
    let NdV = max(dot(N, V), 0.0);
    let NdH = max(dot(N, H), 0.0);
    let HdV = max(dot(H, V), 0.0);

    let f0 = mix(vec3(0.04), albedo, metallic);
    let F = fresnel_schlick(f0, HdV, roughness);

    let specular = cooktorrance_brdf(NdL, NdV, NdH, F, roughness);

    let kS = F;
    var kD = vec3(1.0) - kS;
    kD *= 1.0 - metallic;

    return (kD * albedo / PI + specular) * light_color * attenuation * NdL;
}

fn calculate_ibl(
    albedo: vec3<f32>,
    roughness: f32,
    metallic: f32,
    N: vec3<f32>,
    V: vec3<f32>,
) -> vec3<f32> {
    let NdotV = saturate(dot(N, V));
    let R = reflect(-V, N);

    let kS = fresnel_schlick(vec3(0.001), NdotV, roughness);
    var kD = vec3(1.0) - kS;
    kD *= 1.0 - metallic;
    let irradiance = textureSample(env_map_diffuse, env_map_sampler, N).rgb;
    let diffuse = irradiance * albedo;

    let prefiltered_color = textureSampleLevel(env_map_specular, env_map_sampler, R, roughness * f32(textureNumLevels(env_map_specular))).rgb;
    let brdf = textureSample(env_map_brdf, env_map_sampler, vec2(NdotV, roughness)).rg;
    let specular = prefiltered_color * (kS * brdf.x + brdf.y);

    return kD * diffuse + specular;
}


struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_binormal: vec3<f32>,
    @location(3) world_tangent: vec3<f32>,
    @location(4) uv: vec2<f32>,
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    let world_position = (model_transform * vec4<f32>(input.position, 1.0));

    output.world_position = world_position.xyz;
    output.clip_position = camera.proj * camera.view * world_position;
    output.uv = input.uv;

    var N = normalize((model_transform * vec4<f32>(input.normal, 0.0)).xyz);
    var T = normalize((model_transform * vec4<f32>(input.tangent, 0.0)).xyz);
    var B = normalize(cross(N, T));

    output.world_tangent = T;
    output.world_binormal = B;
    output.world_normal = N;

    return output;
}

@fragment
fn fs_main(vertex: VertexOutput) -> FragmentOutput {
    var output: FragmentOutput;

    let uv = vertex.uv * material.properties.w;

    let tex_color = textureSample(diffuse_tex, diffuse_sampler, uv).rgba * material.base_color.rgba;
    let albedo = pow(tex_color, vec4(2.2));


    var tex_normal = textureSample(normal_tex, normal_sampler, uv).rgb;
    var N: vec3<f32>;
    if tex_normal.r == 0.0 && tex_normal.g == 0.0 && tex_normal.b == 0.0 {
        N = vertex.world_normal;
    } else {
        tex_normal = normalize(tex_normal * 2.0 - 1.0);
        N = normalize(
            vertex.world_tangent * tex_normal.r + vertex.world_binormal * tex_normal.g + vertex.world_normal * tex_normal.b
        );
    }

    let V = normalize(camera.camera_position - vertex.world_position);

    let metallic_roughness = textureSample(roughness_tex, roughness_sampler, uv);
    let roughness = metallic_roughness.g * material.properties.y;
    let metallic = metallic_roughness.b * material.properties.x;

    var illumination = vec3(0.0);

    for (var i = 0u; i < arrayLength(&point_lights); i = i + 1u) {
        let light = point_lights[i];
        if light.intensity == 0.0 {
            continue;
        }

        let light_pos = light.position.xyz;
        let L = normalize(light_pos - vertex.world_position);

        let distance = length(light_pos - vertex.world_position);
        let attenuation = light.intensity / (1.0 + distance * distance / (light.radius * light.radius));

        illumination += calculate_lighting(albedo.rgb, roughness, metallic, N, L, V, light.color.rgb, attenuation);
    }

    let tex_ao = textureSample(ao_tex, ao_sampler, uv).r * material.properties.z;
    let ibl = calculate_ibl(albedo.rgb, roughness, metallic, N, V);
    let ambient = ibl * tex_ao;

    var out_color = ambient + illumination;

    // gamma correction
    out_color = pow(out_color, vec3(1.0 / 2.2));

    output.color = vec4<f32>(out_color, tex_color.a);

    return output;
}
