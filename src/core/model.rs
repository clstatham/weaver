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
                    });
                }

                let index_reader = reader.read_indices().unwrap().into_u32();
                for index in index_reader {
                    indices.push(index);
                }
            }
        }

        let texture = if let Some(image) = images.into_iter().next() {
            Texture::from_data_r8g8b8(
                image.width as usize,
                image.height as usize,
                &image.pixels,
                device,
                queue,
                Some("GLTF Mesh Texture"),
            )
        } else {
            todo!("load default texture")
        };

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

        let material = Material::new().with_base_color_texture(texture);
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
