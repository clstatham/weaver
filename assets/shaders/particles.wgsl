#define_import_path weaver::particles
#import weaver::common::CameraUniform

struct Particle {
    position: vec4<f32>,
    color: vec4<f32>,
}

struct ParticleVertexInput {
    @builtin(instance_index) index: u32,
    @location(0) position: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

struct ParticleVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) index: u32,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
}

@group(0) @binding(0) var<storage> particles: array<Particle>;
@group(0) @binding(1) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(2) @binding(0) var tex_sampler: sampler;

@vertex
fn vs_main(input: ParticleVertexInput) -> ParticleVertexOutput {
    var output: ParticleVertexOutput;

    let particle = particles[input.index];

    // generate lookat matrix (here the origin is our particle position and the "look at" is the camera)
    let forward = normalize(particle.position.xyz - camera.camera_position.xyz);
    let up = vec3<f32>(0.0, 1.0, 0.0);
    let right = cross(up, forward);
    let lookat = mat4x4<f32>(
        vec4<f32>(right, 0.0),
        vec4<f32>(up, 0.0),
        vec4<f32>(forward, 0.0),
        vec4<f32>(0.0, 0.0, 0.0, 1.0)
    );

    // calculate the particle's position
    let position = lookat * vec4<f32>(input.position.xyz, 1.0) + particle.position;

    output.clip_position = camera.proj * camera.view * position;
    output.index = input.index;
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input: ParticleVertexOutput) -> @location(0) vec4<f32> {
    let particle = particles[input.index];
    var color: vec4<f32> = textureSample(tex, tex_sampler, input.uv);
    color = color * particle.color;
    return color;
}