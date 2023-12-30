use std::path::Path;

use weaver_proc_macro::Bundle;
use wgpu::util::DeviceExt;

use super::{
    material::Material,
    mesh::{Mesh, Vertex},
    texture::Texture,
    transform::Transform,
};

#[derive(Bundle)]
pub struct Model {
    pub mesh: Mesh,
    pub transform: Transform,
    pub material: Material,
}

impl Model {
    pub fn load_gltf(
        path: impl AsRef<Path>,
        renderer: &crate::renderer::Renderer,
        use_texture: bool,
    ) -> anyhow::Result<Self> {
        let path = path;
        let device = &renderer.device;
        let queue = &renderer.queue;
        let (document, buffers, images) = gltf::import(path.as_ref())?;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for mesh in document.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                let positions = reader.read_positions().unwrap();
                let normals = reader.read_normals().unwrap();
                let uvs = reader.read_tex_coords(0).unwrap().into_f32();

                for (position, normal, uv) in itertools::multizip((positions, normals, uvs)) {
                    vertices.push(Vertex {
                        position: glam::Vec3::from(position),
                        normal: glam::Vec3::from(normal),
                        uv: glam::Vec2::from(uv),
                        binormal: glam::Vec3::ZERO,
                        tangent: glam::Vec3::ZERO,
                        bitangent: glam::Vec3::ZERO,
                    });
                }

                let index_reader = reader.read_indices().unwrap().into_u32();
                for index in index_reader {
                    indices.push(index);
                }
            }
        }

        let texture = if use_texture {
            if let Some(image) = images.into_iter().next() {
                Some(Texture::from_data_r8g8b8(
                    image.width as usize,
                    image.height as usize,
                    &image.pixels,
                    device,
                    queue,
                    Some("GLTF Mesh Texture"),
                    false,
                ))
            } else {
                None
            }
        } else {
            None
        };

        // calculate tangent, binormal, bitangent

        for c in indices.chunks(3) {
            let i0 = c[0] as usize;
            let i1 = c[1] as usize;
            let i2 = c[2] as usize;

            let v0 = vertices[i0].position;
            let v1 = vertices[i1].position;
            let v2 = vertices[i2].position;

            let uv0 = vertices[i0].uv;
            let uv1 = vertices[i1].uv;
            let uv2 = vertices[i2].uv;

            let delta_pos1 = v1 - v0;
            let delta_pos2 = v2 - v0;

            let delta_uv1 = uv1 - uv0;
            let delta_uv2 = uv2 - uv0;

            let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
            let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;
            let bitangent = (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * r;

            vertices[i0].tangent += tangent;
            vertices[i1].tangent += tangent;
            vertices[i2].tangent += tangent;

            vertices[i0].bitangent += bitangent;
            vertices[i1].bitangent += bitangent;
            vertices[i2].bitangent += bitangent;
        }

        for vertex in vertices.iter_mut() {
            vertex.tangent = vertex.tangent.normalize();
            vertex.bitangent = vertex.bitangent.normalize();
            vertex.binormal = vertex.normal.cross(vertex.tangent).normalize();
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let num_indices = indices.len() as u32;

        let material = Material::new(texture, None, None);
        let transform = Transform::new();

        Ok(Self {
            mesh: Mesh {
                vertex_buffer,
                index_buffer,
                num_indices,
            },
            transform,
            material,
        })
    }
}
