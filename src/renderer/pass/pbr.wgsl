//#import "src/renderer/pass/common.wgsl"

@group(0) @binding(0)  var<uniform> model_transform: mat4x4<f32>;
@group(0) @binding(1)  var<uniform> camera: CameraUniform;
@group(0) @binding(2)  var<uniform> material: MaterialUniform;
@group(0) @binding(3)  var          tex: texture_2d<f32>;
@group(0) @binding(4)  var          tex_sampler: sampler;
@group(0) @binding(5)  var          normal_tex: texture_2d<f32>;
@group(0) @binding(6)  var          normal_tex_sampler: sampler;
@group(0) @binding(7)  var          roughness_tex: texture_2d<f32>;
@group(0) @binding(8)  var          roughness_tex_sampler: sampler;
@group(0) @binding(9)  var          ao_tex: texture_2d<f32>;
@group(0) @binding(10) var          ao_tex_sampler: sampler;
@group(0) @binding(11) var<storage> point_lights: PointLights;
@group(0) @binding(12) var<storage> directional_lights: DirectionalLights;

// group 1 binding 0 is the camera uniform buffer that we already have
@group(1) @binding(1)  var          env_map: texture_cube<f32>;
@group(1) @binding(2)  var          env_map_sampler: sampler;

fn fresnel_factor(F0: vec3<f32>, product: f32) -> vec3<f32> {
    return mix(F0, vec3(1.0), pow(1.01 - product, 5.0));
}

fn d_ggx(NdH: f32, roughness: f32) -> f32 {
    let m = roughness * roughness;
    let m2 = m * m;
    let d = (NdH * m2 - NdH) * NdH + 1.0;
    return m2 / (PI * d * d);
}

fn g_schlick(roughness: f32, NdV: f32, NdL: f32) -> f32 {
    let k = roughness * roughness * 0.5;
    let V = NdV * (1.0 - k) + k;
    let L = NdL * (1.0 - k) + k;
    return 0.25 / (V * L);
}

fn cooktorrance_specular(NdL: f32, NdV: f32, NdH: f32, specular: vec3<f32>, roughness: f32) -> vec3<f32> {
    let D = d_ggx(NdH, roughness);
    let G = g_schlick(roughness, NdV, NdL);

    // todo: get this from the material
    let rim_amount = 0.5;

    let rim = mix(1.0 - roughness * rim_amount * 0.9, 1.0, NdV);
    return (1.0 / rim) * specular * G * D;
}

// based on https://gist.github.com/galek/53557375251e1a942dfa
fn calculate_lighting(
    vertex: VertexOutput,
    albedo: vec3<f32>,
    normal: vec3<f32>,
    light_direction: vec3<f32>,
    view_direction: vec3<f32>,
    light_color: vec3<f32>,
    light_intensity: f32,
    attenuation: f32,
) -> vec3<f32> {
    let metallic = material.properties.x;
    // roughness mapping
    let roughness = textureSample(roughness_tex, roughness_tex_sampler, vertex.uv).r * material.properties.y;

    let N = normalize(normal);
    let V = normalize(view_direction);
    let L = normalize(light_direction);
    let H = normalize(V + L);
    let NB = normalize(vertex.world_binormal);
    let NT = normalize(vertex.world_tangent);

    let tnrm = transpose(mat3x3<f32>(
        vertex.world_tangent,
        vertex.world_bitangent,
        vertex.world_normal
    ));

    let specular = mix(vec3(0.04), albedo, metallic);

    // diffuse IBL
    let env_diffuse = textureSample(env_map, env_map_sampler, tnrm * N).rgb;

    // specular IBL
    let R = reflect(-V, N);
    let env_specular = textureSample(env_map, env_map_sampler, tnrm * R).rgb;

    let NdL = max(dot(N, L), 0.0);
    let NdV = max(dot(N, V), 0.001);
    let NdH = max(dot(N, H), 0.001);
    let HdV = max(dot(H, V), 0.001);
    let LdV = max(dot(L, V), 0.001);

    // specular reflectance
    let spec_fresnel = fresnel_factor(specular, HdV);
    let spec_ref = cooktorrance_specular(NdL, NdV, NdH, spec_fresnel, roughness) * NdL;

    // diffuse
    let diff_ref = (1.0 - spec_fresnel) * (1.0 / PI) * NdL;

    var reflected_light = vec3(0.0);
    var diffuse_light = vec3(0.0);

    // direct lighting
    let direct_light = light_color * light_intensity * attenuation;
    reflected_light += direct_light * spec_ref;
    diffuse_light += direct_light * diff_ref;

    // IBL lighting
    // todo: update this when we have a proper IBL map
    let indirect_light = env_diffuse + env_specular;
    reflected_light += indirect_light * spec_fresnel;
    diffuse_light += indirect_light * (1.0 - spec_fresnel);

    let result = (diffuse_light * mix(albedo, vec3(0.0), metallic)) + reflected_light;

    return result;
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
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
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    let texture_scale = material.texture_scale.x;
    let uv = vertex.uv * texture_scale;

    // sample color texture and normal map
    let tex_color = textureSample(tex, tex_sampler, uv).xyz;
    let tex_ao = textureSample(ao_tex, ao_tex_sampler, uv).r;
    var tex_normal = textureSample(normal_tex, normal_tex_sampler, uv).xyz;
    tex_normal = normalize(tex_normal * 2.0 - 1.0);

    // create TBN matrix
    let TBN = mat3x3<f32>(
        vertex.world_tangent,
        vertex.world_bitangent,
        vertex.world_normal
    );

    // transform normal from tangent space to world space
    let normal = normalize(TBN * tex_normal);

    let view_direction = normalize(camera.camera_position - vertex.world_position);

    // calculate lighting for all lights
    var illumination = vec3<f32>(0.0, 0.0, 0.0);

    for (var i = 0u; i < point_lights.count; i = i + 1u) {
        let light = point_lights.lights[i];
        let light_pos = light.position.xyz;
        let light_direction = normalize(light_pos - vertex.world_position);
        let attenuation = 20.0 / length(light_pos - vertex.world_position);
        illumination += calculate_lighting(vertex, material.base_color.xyz, normal, light_direction, view_direction, light.color.xyz, light.intensity, attenuation);
    }

    for (var i = 0u; i < directional_lights.count; i = i + 1u) {
        let light = directional_lights.lights[i];
        let light_direction = light.direction.xyz;
        let attenuation = 1.0;
        illumination += calculate_lighting(vertex, material.base_color.xyz, normal, light_direction, view_direction, light.color.xyz, light.intensity, attenuation);
    }

    let out_color = tex_color * tex_ao * illumination;

    return vec4<f32>(out_color, 1.0);
}
