#define_import_path weaver::picking

struct PickResult {
    position: vec4<f32>,
    normal: vec4<f32>,
}

struct Camera {
    inv_view_proj: mat4x4<f32>,
}

@group(0) @binding(0) var<storage, read_write> result: PickResult;
@group(0) @binding(1) var<uniform> camera: Camera;
@group(0) @binding(2) var depth_texture: texture_depth_2d;
@group(0) @binding(3) var normal_texture: texture_2d<f32>;
@group(0) @binding(4) var<uniform> screen_position: vec2<f32>;

@compute
@workgroup_size(1, 1, 1)
fn main() {
    let depth = textureLoad(depth_texture, vec2<i32>(i32(screen_position.x), i32(screen_position.y)), 0);
    let normal = textureLoad(normal_texture, vec2<i32>(i32(screen_position.x), i32(screen_position.y)), 0);

    let dims = vec2<f32>(textureDimensions(depth_texture));
    let ndc = vec4<f32>(
        (screen_position.x / dims.x) * 2.0 - 1.0,
        (1.0 - screen_position.y / dims.y) * 2.0 - 1.0,
        depth * 2.0 - 1.0,
        1.0
    );

    var world_pos = camera.inv_view_proj * ndc;
    world_pos /= world_pos.w;

    result.position = world_pos;
    result.normal = normal;
}

