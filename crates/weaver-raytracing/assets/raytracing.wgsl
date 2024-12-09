const PI: f32 = 3.14159265359;
const BIAS: f32 = 0.0001;
const RAYS_PER_PIXEL: u32 = 500;
const BOUNCES: u32 = 100;

struct Camera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    camera_position: vec3<f32>,
    _padding: u32,
}

struct Material {
    albedo: vec4<f32>,
    emission: vec4<f32>,
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

var<push_constant> SCREEN_DIMS: vec2<u32>;

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<storage> objects: array<Object>;
@group(2) @binding(0) var<storage> seed_buf: array<f32>;

// https://www.pcg-random.org/
fn pcg(n: u32) -> u32 {
    var h = n * 747796405u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    return (h >> 22u) ^ h;
}

// http://www.jcgt.org/published/0009/03/02/
fn pcg3d(p: vec3<u32>) -> vec3<u32> {
    var v = p * 1664525u + 1013904223u;
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    v ^= v >> vec3<u32>(16u);
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    return v;
}

fn rand(f: ptr<function, f32>) -> f32 {
    *f = f32(pcg(bitcast<u32>(*f))) / f32(0xffffffff);
    return *f;
}

fn randn(f: ptr<function, f32>) -> f32 {
    let theta = 2.0 * PI * rand(f);
    let rho = sqrt(-2.0 * log(rand(f)));
    return rho * cos(theta);
}

fn unproject(clip: vec3<f32>) -> vec3<f32> {
    var eye = camera.inv_proj * vec4(clip, 1.0);
    eye /= eye.w;
    return (camera.inv_view * eye).xyz;
}

fn ray(uv: vec2<f32>, seed: ptr<function, f32>) -> Ray {
    let clip = vec3(uv, 1.0);
    let world = unproject(clip);
    let origin = camera.camera_position;
    let direction = normalize(world - origin);
    return Ray(origin, direction);
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
    material: Material,
}

fn no_intersection() -> Intersection {
    return Intersection(false, vec3(0.0), vec3(0.0), Material(vec4(0.0), vec4(0.0)));
}

fn intersect_scene(ray: Ray) -> Intersection {
    var closest_distance = 1000000.0;
    var closest_intersection = no_intersection();

    for (var i = 0u; i < arrayLength(&objects); i = i + 1u) {
        let object = objects[i];
        let position = object.model_transform[3];

        let oc = ray.origin - position.xyz;
        let a = dot(ray.direction, ray.direction);
        let b = 2.0 * dot(oc, ray.direction);
        let c = dot(oc, oc) - object.radius * object.radius;
        let roots = solve_quadratic(a, b, c);
        if !roots.valid {
            continue;
        }

        let distance = min(roots.x1, roots.x2);

        if distance < closest_distance && distance > 0.0 {
            let hit_position = ray.origin + ray.direction * distance;
            let normal = normalize(hit_position - position.xyz);

            closest_distance = distance;
            closest_intersection = Intersection(true, hit_position, normal, object.material);
        }
    }

    return closest_intersection;
}

fn random_direction(seed: ptr<function, f32>) -> vec3<f32> {
    return normalize(vec3(randn(seed), randn(seed), randn(seed)));
}

fn sample_hemisphere(normal: vec3<f32>, seed: ptr<function, f32>) -> vec3<f32> {
    let u = rand(seed);
    let v = rand(seed);
    let theta = 2.0 * PI * u;
    let phi = acos(2.0 * v - 1.0);
    let x = sin(phi) * cos(theta);
    let y = sin(phi) * sin(theta);
    let z = cos(phi);
    let sample = vec3(x, y, z);
    return normalize(sample);
}

fn integrate(ray: Ray, seed: ptr<function, f32>) -> vec3<f32> {
    var incoming_light = vec3<f32>(0.0, 0.0, 0.0);
    var ray_color = vec3<f32>(1.0, 1.0, 1.0);

    let intersection = intersect_scene(ray);
    if !intersection.hit {
        return vec3<f32>(0.0, 0.0, 0.0);
    }

    var hit_position = intersection.position;
    var normal = intersection.normal;
    var material = intersection.material;

    for (var i = 0u; i < BOUNCES; i = i + 1u) {
        let new_ray = Ray(hit_position + normal * BIAS, sample_hemisphere(normal, seed));
        let new_intersection = intersect_scene(new_ray);
        if (!new_intersection.hit) {
            incoming_light += material.emission.xyz;
            break;
        }

        let new_hit_position = new_intersection.position;
        let new_normal = new_intersection.normal;
        let new_material = new_intersection.material;

        let cos_theta = dot(normal, new_ray.direction);
        let brdf = material.albedo.xyz / PI;

        incoming_light += ray_color * material.emission.xyz;
        incoming_light += ray_color * brdf * max(0.0, cos_theta);

        ray_color *= new_material.albedo.xyz;
        hit_position = new_hit_position;
        normal = new_normal;
        material = new_material;
    }

    return incoming_light * ray_color;
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
    var quad = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );

    uv = quad[vertex_index];

    let out = vec4<f32>(uv, 0.0, 1.0);

    return VertexOutput(out, uv);
}


@fragment
fn raytracing_fs_main(
    vertex_output: VertexOutput,
) -> @location(0) vec4<f32> {
    let screen_dims = vec2<f32>(f32(SCREEN_DIMS.x), f32(SCREEN_DIMS.y));
    var uv = (vertex_output.uv + vec2<f32>(1.0)) / 2.0;
    uv *= screen_dims;
    let index = u32(uv.x) + u32(uv.y) * SCREEN_DIMS.x;
    var seed = seed_buf[index];
    
    var out_color = vec3<f32>(0.0, 0.0, 0.0);
    for (var i = 0u; i < RAYS_PER_PIXEL; i = i + 1u) {
        var ray = ray(vertex_output.uv, &seed);
        ray.direction = normalize(ray.direction + random_direction(&seed) * 0.001);
        out_color += integrate(ray, &seed);
    }
    out_color /= f32(RAYS_PER_PIXEL);

    return vec4<f32>(out_color, 1.0);
}
