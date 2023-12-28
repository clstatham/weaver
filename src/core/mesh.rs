use weaver_proc_macro::Component;
use wgpu::util::DeviceExt;

use super::{color::Color, model::Model, texture::Texture, Vertex};

#[derive(Component)]
pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub(crate) texture: Texture,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) num_indices: u32,
}

impl Mesh {
    pub fn load_gltf(
        path: impl AsRef<std::path::Path>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        transform_buffer: &wgpu::Buffer,
        camera_transform_buffer: &wgpu::Buffer,
    ) -> anyhow::Result<Mesh> {
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
                    vertices.push(Vertex::new(
                        glam::Vec3::from(position),
                        glam::Vec3::from(normal),
                        Color::WHITE,
                        glam::Vec2::from(uv),
                    ));
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

        let bind_group = Model::bind_group(
            device,
            transform_buffer,
            camera_transform_buffer,
            &texture.view,
            &texture.sampler,
        );

        Ok(Self {
            vertex_buffer,
            index_buffer,
            num_indices,
            texture,
            bind_group,
        })
    }
}
