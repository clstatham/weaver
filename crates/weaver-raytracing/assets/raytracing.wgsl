const PI: f32 = 3.14159265359;
const BIAS: f32 = 0.001;
const SPLITS: u32 = 1;
const BOUNCES: u32 = 3;

struct Camera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    camera_position: vec3<f32>,
    _padding: u32,
}

struct PointLight {
    position: vec3<f32>,
    _padding: u32,
    color: vec4<f32>,
    intensity: f32,
}

struct Material {
    diffuse: vec4<f32>,
}

struct Object {
    model_transform: mat4x4<f32>,
    material: Material,
    radius: f32,
}

struct Ray {
    origin: vec3<f32>,
    direction: vec3<f32>,
}

var<push_constant> SEED: vec3<f32>;

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<storage> point_lights: array<PointLight>;
@group(2) @binding(0) var<storage> objects: array<Object>;
@group(3) @binding(0) var output: texture_storage_2d<rgba16float, write>;

// http://www.jcgt.org/published/0009/03/02/
fn pcg3d(p: vec3<u32>) -> vec3<u32> {
    var v = p * 1664525u + 1013904223u;
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    v ^= v >> vec3<u32>(16u);
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    return v;
}


fn rand33(f: ptr<function, vec3<f32>>) -> vec3<f32> {
    *f = vec3f(pcg3d(bitcast<vec3<u32>>(*f))) / f32(0xffffffff);
    return *f;
}

fn ray(vertex_coords: vec4<f32>) -> Ray {
    let clip = camera.inv_proj * vertex_coords;
    var eye = camera.inv_view * vec4<f32>(clip.xyz, 1.0);
    eye /= eye.w;
    let direction = normalize(eye.xyz - camera.camera_position);
    return Ray(camera.camera_position, direction);
}

struct QuadraticRoots {
    valid: bool,
    x1: f32,
    x2: f32,
}

fn solve_quadratic(a: f32, b: f32, c: f32) -> QuadraticRoots {
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return QuadraticRoots(false, -1.0, -1.0);
    }
    let sqrt_discriminant = sqrt(discriminant);
    let x1 = (-b - sqrt_discriminant) / (2.0 * a);
    let x2 = (-b + sqrt_discriminant) / (2.0 * a);
    return QuadraticRoots(true, x1, x2);
}

struct Intersection {
    hit: bool,
    position: vec3<f32>,
    normal: vec3<f32>,
    color: vec4<f32>,
}


fn intersect_scene(ray: Ray) -> Intersection {
    var closest_distance = 1000000.0;
    var closest_intersection = Intersection(false, vec3(0.0), vec3(0.0), vec4<f32>(0.0, 0.0, 0.0, 1.0));

    for (var i = 0u; i < arrayLength(&objects); i = i + 1u) {
        let object = objects[i];
        let position = object.model_transform * vec4<f32>(0.0, 0.0, 0.0, 1.0);

        let oc = ray.origin - position.xyz;
        let a = dot(ray.direction, ray.direction);
        let b = 2.0 * dot(oc, ray.direction);
        let c = dot(oc, oc) - object.radius * object.radius;
        let roots = solve_quadratic(a, b, c);
        if !roots.valid {
            continue;
        }

        let distance = min(roots.x1, roots.x2);
        let hit_position = ray.origin + ray.direction * distance;
        let normal = normalize(hit_position - position.xyz);
        let color = object.material.diffuse;

        if distance < closest_distance {
            closest_distance = distance;
            closest_intersection = Intersection(true, ray.origin + ray.direction * distance, normal, color);
        }
    }

    return closest_intersection;
}

fn sample_lights(intersection: Intersection) -> vec3<f32> {
    var color = vec3<f32>(0.0, 0.0, 0.0);

    for (var i = 0u; i < arrayLength(&point_lights); i = i + 1u) {
        let light = point_lights[i];
        let light_direction = light.position - intersection.position;
        let light_distance = length(light_direction);
        let light_normal = normalize(light_direction);

        // let shadow_origin = intersection.position + light_normal * BIAS;
        // let shadow_ray = Ray(shadow_origin, light_normal);
        // let shadow_intersection = intersect_scene(shadow_ray);

        // if !shadow_intersection.hit {
        let diffuse = max(dot(intersection.normal, light_normal), 0.0);
        let light_intensity = light.intensity * light.color.rgb / (4.0 * PI * light_distance * light_distance);
        color += light_intensity * diffuse;
        // }
    }

    return color;
}

fn scatter_ray(ray: Ray, seed: ptr<function, vec3<f32>>) -> Ray {
    let offset = (rand33(seed) * 2.0 - 1.0) * 0.0001;
    let new_direction = normalize(ray.direction + offset);
    return Ray(ray.origin + new_direction * BIAS, new_direction);
}

fn integrate(ray: Ray, bounces: u32, seed: ptr<function, vec3<f32>>) -> vec3<f32> {
    var new_ray = scatter_ray(ray, seed);
    var intersection = intersect_scene(new_ray);
    if !intersection.hit {
        return vec3<f32>(0.0, 0.0, 0.0);
    }

    var color = intersection.color.rgb * sample_lights(intersection);

    for (var b = 0u; b < bounces; b = b + 1u) {
        new_ray = Ray(intersection.position, intersection.normal);
        new_ray = scatter_ray(new_ray, seed);
        let new_intersection = intersect_scene(new_ray);
        if !new_intersection.hit {
            break;
        }

        color += new_intersection.color.rgb * sample_lights(new_intersection);
        intersection = new_intersection;
    }

    return color / f32(bounces);
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn raytracing_vs_main(
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var uv: vec2<f32>;
    uv.x = f32((vertex_index << 1u) & 2u);
    uv.y = f32(vertex_index & 2u);
    uv = uv * 2.0 - 1.0;
    var out = vec4<f32>(uv, 0.0, 1.0);

    return VertexOutput(out, uv);
}


@fragment
fn raytracing_fs_main(
    vertex_output: VertexOutput,
) -> @location(0) vec4<f32> {
    var seed = SEED;
    let ray = ray(vec4(vertex_output.uv, 0.0, 1.0));
    var out_color = vec3<f32>(0.0, 0.0, 0.0);
    for (var i = 0u; i < SPLITS; i = i + 1u) {
        out_color += integrate(ray, BOUNCES, &seed);
    }
    out_color /= f32(SPLITS);

    return vec4<f32>(out_color, 1.0);
}
