use wgpu::util::DeviceExt;

use super::{color::Color, Vertex};

pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub(crate) num_indices: u32,
}

impl Mesh {
    pub fn load_gltf(
        path: impl AsRef<std::path::Path>,
        device: &wgpu::Device,
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
                    vertices.push(Vertex {
                        position: position.into(),
                        color: Color::WHITE,
                        normal: normal.into(),
                        uv: uv.into(),
                    });
                }

                let index_reader = reader.read_indices().unwrap().into_u32();
                for index in index_reader {
                    indices.push(index);
                }
            }
        }

        // // load texture
        // let texture = if let Some(image) = images.into_iter().next() {
        //     log::info!("Loading texture");
        //     let texture = Texture::from_data_r8g8b8(
        //         image.width as usize,
        //         image.height as usize,
        //         &image.pixels,
        //     );
        //     Some(texture)
        // } else {
        //     None
        // };

        // Ok(Mesh::new(vertices, indices, texture))

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

        Ok(Self {
            vertex_buffer,
            index_buffer,
            num_indices,
        })
    }
}
