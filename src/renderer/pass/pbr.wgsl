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

struct PointLight {
    position: vec3<f32>,
    _pad1: u32,
    color: vec3<f32>,
    _pad2: u32,
    intensity: f32,
    _pad3: array<u32, 3>,
};

struct DirectionalLight {
    direction: vec3<f32>,
    _pad1: u32,
    color: vec3<f32>,
    _pad2: u32,
    intensity: f32,
    _pad3: array<u32, 3>,
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
@group(0) @binding(9)  var<storage> point_lights: array<PointLight>;
@group(0) @binding(10) var<storage> directional_lights: array<DirectionalLight>;

fn frensel_factor(f0: vec3<f32>, product: f32) -> vec3<f32> {
    return mix(f0, vec3<f32>(1.0, 1.0, 1.0), pow(1.01 - product, 5.0));
}

fn phong_specular(v: vec3<f32>, l: vec3<f32>, n: vec3<f32>, specular: vec3<f32>, roughness: f32) -> vec3<f32> {
    let r = reflect(-l, n);
    let spec = max(dot(r, v), 0.0);
    let k = 1.999 / (roughness * roughness);
    return min(1.0, 3.0 * 0.0398 * k) * pow(spec, min(10000.0, k)) * specular;
}

fn calculate_lighting(
    vertex: VertexOutput,
    tex_color: vec3<f32>,
    tex_normal: vec3<f32>,
    light_direction: vec3<f32>,
    light_color: vec3<f32>,
    light_intensity: f32
) -> vec3<f32> {
    let metallic = material.metallic.x;

    let l = normalize(light_direction);
    let v = normalize(vertex.world_position - camera.camera_position);
    let h = normalize(l + v);
    let nn = normalize(vertex.world_normal);

    let nb = normalize(cross(vertex.world_normal, vertex.world_tangent));
    let tbn = mat3x3<f32>(nb, vertex.world_tangent, nn);

    // normal mapping
    let n = normalize(tbn * (tex_normal * 2.0 - 1.0));

    // roughness mapping
    let roughness = textureSample(roughness_tex, roughness_tex_sampler, vertex.uv).x * material.metallic.y;

    let base = material.base_color.xyz;

    let specular = mix(vec3<f32>(0.04, 0.04, 0.04), base, metallic);

    // todo: environment cube mapping

    let ndl = max(dot(n, l), 0.0);
    let ndv = max(dot(n, v), 0.001);
    let ndh = max(dot(n, h), 0.001);
    let hdv = max(dot(h, v), 0.001);
    let ldv = max(dot(l, v), 0.001);

    // phong specular
    let specfresnel = frensel_factor(specular, ndv);
    let specref = phong_specular(v, l, n, specfresnel, material.metallic.y) * ndl;

    // diffuse
    let diffref = (1.0 - specfresnel) * (1.0 / PI) * ndl;

    // ambient
    let ambient = vec3<f32>(0.03, 0.03, 0.03) * base;

    let reflected_light = specref * light_color * light_intensity;
    let diffuse_light = diffref * light_color * light_intensity;
    let ambient_light = ambient * light_color * light_intensity;

    return reflected_light + diffuse_light + ambient_light;
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

    let uv = vertex.uv * texture_scale;
    let tex_color = textureSample(tex, tex_sampler, uv).xyz;
    let tex_normal = textureSample(normal_tex, normal_tex_sampler, uv).xyz;

    var out_color = vec3<f32>(0.0, 0.0, 0.0);

    for (var i = 0u; i < arrayLength(&point_lights); i = i + 1u) {
        out_color += calculate_lighting(vertex, point_lights[i].position, tex_color, tex_normal, point_lights[i].color, point_lights[i].intensity);
    }

    for (var i = 0u; i < arrayLength(&directional_lights); i = i + 1u) {
        out_color += calculate_lighting(vertex, directional_lights[i].direction, tex_color, tex_normal, directional_lights[i].color, directional_lights[i].intensity);
    }

    out_color *= tex_color;

    return vec4<f32>(out_color, 1.0);
}
