#define_import_path weaver::common

const PI: f32 = 3.1415926535897932384626433832795;

const MAX_LIGHTS: u32 = 32u;
const MIN_LIGHT_INTENSITY: f32 = 0.01;
const FAR_PLANE: f32 = 100.0;

struct VertexInput {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) uv: vec2<f32>,
};

struct ModelTransform {
    model: mat4x4<f32>,
};


struct CameraUniform {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    camera_position: vec3<f32>,
};

struct MaterialUniform {
    base_color: vec4<f32>,
    properties: vec4<f32>, // x: metallic, y: roughness, z: ao, w: texture_scale
};

struct PointLight {
    position: vec4<f32>,
    color: vec4<f32>,
    proj_transform: mat4x4<f32>,
    intensity: f32,
    radius: f32,
};

struct DirectionalLight {
    direction: vec4<f32>,
    color: vec4<f32>,
    view_transform: mat4x4<f32>,
    proj_transform: mat4x4<f32>,
    intensity: f32,
    _pad: f32,
};

struct PointLights {
    count: u32,
    _pad: array<u32, 3>,
    lights: array<PointLight, MAX_LIGHTS>,
};

struct DirectionalLights {
    count: u32,
    _pad: array<u32, 3>,
    lights: array<DirectionalLight, MAX_LIGHTS>,
};

fn ndc_cube() -> array<vec3<f32>, 36> {
    return array<vec3<f32>, 36>(
        // right
        vec3<f32>(1.0, -1.0, -1.0),
        vec3<f32>(1.0, -1.0, 1.0),
        vec3<f32>(1.0, 1.0, 1.0),
        vec3<f32>(1.0, 1.0, 1.0),
        vec3<f32>(1.0, 1.0, -1.0),
        vec3<f32>(1.0, -1.0, -1.0),

        // left
        vec3<f32>(-1.0, -1.0, 1.0),
        vec3<f32>(-1.0, -1.0, -1.0),
        vec3<f32>(-1.0, 1.0, -1.0),
        vec3<f32>(-1.0, 1.0, -1.0),
        vec3<f32>(-1.0, 1.0, 1.0),
        vec3<f32>(-1.0, -1.0, 1.0),

        // top
        vec3<f32>(-1.0, 1.0, -1.0),
        vec3<f32>(-1.0, 1.0, 1.0),
        vec3<f32>(1.0, 1.0, 1.0),
        vec3<f32>(1.0, 1.0, 1.0),
        vec3<f32>(1.0, 1.0, -1.0),
        vec3<f32>(-1.0, 1.0, -1.0),

        // bottom
        vec3<f32>(-1.0, -1.0, 1.0),
        vec3<f32>(-1.0, -1.0, -1.0),
        vec3<f32>(1.0, -1.0, -1.0),
        vec3<f32>(1.0, -1.0, -1.0),
        vec3<f32>(1.0, -1.0, 1.0),
        vec3<f32>(-1.0, -1.0, 1.0),

        // front
        vec3<f32>(-1.0, -1.0, 1.0),
        vec3<f32>(1.0, -1.0, 1.0),
        vec3<f32>(1.0, 1.0, 1.0),
        vec3<f32>(1.0, 1.0, 1.0),
        vec3<f32>(-1.0, 1.0, 1.0),
        vec3<f32>(-1.0, -1.0, 1.0),

        // back
        vec3<f32>(1.0, -1.0, -1.0),
        vec3<f32>(-1.0, -1.0, -1.0),
        vec3<f32>(-1.0, 1.0, -1.0),
        vec3<f32>(-1.0, 1.0, -1.0),
        vec3<f32>(1.0, 1.0, -1.0),
        vec3<f32>(1.0, -1.0, -1.0),
    );
}