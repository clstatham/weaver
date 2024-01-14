const PI: f32 = 3.1415926535897932384626433832795;

@group(0) @binding(0) var src: texture_cube<f32>;
@group(0) @binding(1) var cube_sampler: sampler;

@group(1) @binding(0) var<storage> views: array<mat4x4<f32>, 6>;
@group(1) @binding(1) var<storage> projection: mat4x4<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
};

@vertex
fn irradiance_vs_main(@builtin(vertex_index) idx: u32, @builtin(view_index) vidx: i32) -> VertexOutput {
    // NDC cube
    var VERTICES: array<vec3<f32>, 36> = array<vec3<f32>, 36>(
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

    let world_position = VERTICES[u32(vidx) * 6u + idx];
    let view = views[vidx];

    let clip_position = projection * view * vec4<f32>(world_position, 1.0);

    var output: VertexOutput;
    output.clip_position = clip_position;
    output.world_position = vec4<f32>(world_position, 1.0);
    return output;
}

@fragment
fn irradiance_fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    let N = normalize(vertex.world_position.xyz);
    var up = vec3<f32>(0.0, 1.0, 0.0);
    let right = cross(up, N);
    up = cross(N, right);
    let kernel_size = 16.0;
    let sample_delta = 1.0 / kernel_size;
    var total_samples = 0.0;
    var result = vec3<f32>(0.0, 0.0, 0.0);
    for (var phi: f32 = 0.0; phi < 2.0 * PI; phi = phi + sample_delta) {
        for (var theta: f32 = 0.0; theta < 0.5 * PI; theta = theta + sample_delta) {
            let tangent_sample = vec3<f32>(sin(theta) * cos(phi), sin(theta) * sin(phi), cos(theta));
            let sample = tangent_sample.x * right + tangent_sample.y * up + tangent_sample.z * N;
            let weight = cos(theta) * sin(theta);
            result += textureSample(src, cube_sampler, sample).rgb * weight;
            total_samples += 1.0;
        }
    }
    return vec4<f32>(result * PI / total_samples, 1.0);
}
