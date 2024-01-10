//#import "src/renderer/pass/common.wgsl"

// model information
@group(0) @binding(0) var<storage> model_transforms: array<mat4x4<f32>>;

// envorinment information
@group(1) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(1) var          env_map: texture_cube<f32>;
@group(1) @binding(2) var          env_sampler: sampler;

// material information
@group(2) @binding(0) var<uniform> material: MaterialUniform;
@group(2) @binding(1) var          diffuse_tex: texture_2d<f32>;
@group(2) @binding(2) var          normal_tex: texture_2d<f32>;
@group(2) @binding(3) var          roughness_tex: texture_2d<f32>;
@group(2) @binding(4) var          ao_tex: texture_2d<f32>;
@group(2) @binding(5) var          tex_sampler: sampler;

// light information
@group(3) @binding(0) var<storage> point_lights: PointLights;

fn saturate(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

fn fresnel_schlick(f0: vec3<f32>, HdV: f32, roughness: f32) -> vec3<f32> {
    return f0 + (max(vec3(1.0 - roughness), f0) - f0) * pow(saturate(1.0 - HdV), 5.0);
}

fn d_ggx(NdH: f32, roughness: f32) -> f32 {
    let m = roughness * roughness;
    let m2 = m * m;
    let d = (NdH * m2 - NdH) * NdH + 1.0;
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


// based on https://gist.github.com/galek/53557375251e1a942dfa
fn calculate_lighting(
    vertex: VertexOutput,
    albedo: vec3<f32>,
    roughness: f32,
    metallic: f32,
    normal: vec3<f32>,
    light_direction: vec3<f32>,
    view_direction: vec3<f32>,
    light_color: vec3<f32>,
    attenuation: f32,
) -> vec3<f32> {
    let N = normalize(normal);
    let V = normalize(view_direction);
    let L = normalize(light_direction);
    let H = normalize(V + L);

    let NdL = max(dot(N, L), 0.0);
    let NdV = max(dot(N, V), 0.001);
    let NdH = max(dot(N, H), 0.001);
    let HdV = max(dot(H, V), 0.001);

    let f0 = mix(vec3(0.04), albedo, metallic);
    let F = fresnel_schlick(f0, HdV, roughness);

    let specular = cooktorrance_brdf(NdL, NdV, NdH, F, roughness);

    let kS = F;
    var kD = vec3(1.0) - kS;
    kD *= 1.0 - metallic;

    return (kD * albedo / PI + specular) * light_color * attenuation * NdL;
}


fn calculate_ibl(
    vertex: VertexOutput,
    albedo: vec3<f32>,
    normal: vec3<f32>,
    view_direction: vec3<f32>,
    roughness: f32,
    metallic: f32,
) -> vec3<f32> {
    let N = normalize(normal);
    let V = normalize(view_direction);
    let NdV = max(dot(N, V), 0.001);
    let R = reflect(-V, N);

    let diffuse_irradiance = textureSample(env_map, env_sampler, N).rgb;

    let diffuse = diffuse_irradiance * albedo;

    // let specular_irradiance = textureSample(env_map, env_sampler, R).rgb;
    // let prefiltered_color = textureSampleLod(env_map, env_sampler, R, roughness * 8.0).rgb;
    // let brdf = textureSample(brdf_lut, brdf_sampler, vec2(NdV, roughness)).rgb;

    // let specular = (prefiltered_color * (specular_irradiance * brdf.x + brdf.y)) * metallic;

    // return diffuse + specular;

    return diffuse;
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_binormal: vec3<f32>,
    @location(3) world_tangent: vec3<f32>,
    @location(4) world_bitangent: vec3<f32>,
    @location(5) uv: vec2<f32>,
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    let model_transform = model_transforms[input.instance_index];

    output.world_position = (model_transform * vec4<f32>(input.position, 1.0)).xyz;
    output.clip_position = camera.proj * camera.view * vec4<f32>(output.world_position, 1.0);
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
fn fs_main(vertex: VertexOutput) -> FragmentOutput {
    var output: FragmentOutput;

    let tex_color = textureSample(diffuse_tex, tex_sampler, material.texture_scale.x * vertex.uv).xyz;
    let albedo = pow(tex_color, vec3(2.2));

    var tex_normal = textureSample(normal_tex, tex_sampler, material.texture_scale.x * vertex.uv).xyz;
    // transform normal from tangent space to world space
    let normal = normalize(vertex.world_tangent * tex_normal.x + vertex.world_binormal * tex_normal.y + vertex.world_normal * tex_normal.z);

    let metallic_roughness = textureSample(roughness_tex, tex_sampler, material.texture_scale.x * vertex.uv);
    let roughness = metallic_roughness.g * material.properties.y;
    let metallic = metallic_roughness.b * material.properties.x;

    let view_direction = normalize(camera.camera_position - vertex.world_position);

    var illumination = vec3(0.0);

    for (var i = 0u; i < point_lights.count; i = i + 1u) {
        let light = point_lights.lights[i];
        let light_pos = light.position.xyz;
        let light_direction = normalize(light_pos - vertex.world_position);

        let distance = length(light_pos - vertex.world_position);
        let attenuation = light.intensity / (1.0 + distance * distance / (light.radius * light.radius));

        if attenuation < MIN_LIGHT_INTENSITY {
            // light is too far away, ignore it
            continue;
        }
        illumination += calculate_lighting(vertex, albedo, roughness, metallic, normal, light_direction, view_direction, light.color.xyz, attenuation);
    }

    let tex_ao = textureSample(ao_tex, tex_sampler, material.texture_scale.x * vertex.uv).r;

    // WIP
    // illumination += calculate_ibl(vertex, albedo, normal, view_direction, roughness, metallic);

    var out_color = illumination * tex_ao;

    // gamma correction
    out_color = pow(out_color, vec3(1.0 / 2.2));
    out_color = clamp(out_color, vec3(0.0), vec3(1.0));
    output.color = vec4<f32>(out_color, 1.0);

    return output;
}
