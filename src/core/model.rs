use std::path::Path;

use rustc_hash::FxHashMap;
use weaver_proc_macro::Bundle;
use wgpu::util::DeviceExt;

use crate::renderer::Renderer;

use super::{
    material::Material,
    mesh::{Mesh, Vertex},
    texture::Texture,
    transform::Transform,
};

fn calculate_tangents(vertices: &mut [Vertex], indices: &[u32]) {
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
}

#[derive(Bundle)]
pub struct Model {
    pub mesh: Mesh,
    pub transform: Transform,
    pub material: Material,
}

impl Model {
    pub fn load_gltf(
        path: impl AsRef<Path>,
        renderer: &Renderer,
        use_material: bool,
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

        let mut mat = Material::default();

        if use_material {
            for material in document.materials() {
                if let Some(texture) = material.pbr_metallic_roughness().base_color_texture() {
                    let image = images.get(texture.texture().source().index()).unwrap();
                    match image.format {
                        gltf::image::Format::R8G8B8 => {
                            mat.diffuse_texture = Some(Texture::from_data_r8g8b8(
                                image.width as usize,
                                image.height as usize,
                                &image.pixels,
                                device,
                                queue,
                                Some("GLTF Mesh Diffuse Texture"),
                                false,
                            ));
                        }
                        gltf::image::Format::R8G8B8A8 => {
                            mat.diffuse_texture = Some(Texture::from_data_rgba8(
                                image.width as usize,
                                image.height as usize,
                                &image.pixels,
                                device,
                                queue,
                                Some("GLTF Mesh Diffuse Texture"),
                                false,
                            ));
                        }
                        _ => {
                            todo!("Unsupported GLTF Texture Format");
                        }
                    }
                } else {
                    log::warn!("GLTF Mesh has no diffuse texture");
                    mat.diffuse_texture = Some(Texture::default_texture(device, queue));
                }
                if let Some(texture) = material.normal_texture() {
                    let image = images.get(texture.texture().source().index()).unwrap();
                    match image.format {
                        gltf::image::Format::R8G8B8 => {
                            mat.normal_texture = Some(Texture::from_data_r8g8b8(
                                image.width as usize,
                                image.height as usize,
                                &image.pixels,
                                device,
                                queue,
                                Some("GLTF Mesh Normal Texture"),
                                true,
                            ));
                        }
                        gltf::image::Format::R8G8B8A8 => {
                            mat.normal_texture = Some(Texture::from_data_rgba8(
                                image.width as usize,
                                image.height as usize,
                                &image.pixels,
                                device,
                                queue,
                                Some("GLTF Mesh Normal Texture"),
                                true,
                            ));
                        }
                        _ => {
                            todo!("Unsupported GLTF Texture Format");
                        }
                    }
                } else {
                    log::warn!("GLTF Mesh has no normal texture");
                }
                if let Some(texture) = material
                    .pbr_metallic_roughness()
                    .metallic_roughness_texture()
                {
                    let image = images.get(texture.texture().source().index()).unwrap();
                    match image.format {
                        gltf::image::Format::R8G8B8 => {
                            mat.roughness_texture = Some(Texture::from_data_r8g8b8(
                                image.width as usize,
                                image.height as usize,
                                &image.pixels,
                                device,
                                queue,
                                Some("GLTF Mesh Roughness Texture"),
                                false,
                            ));
                        }
                        gltf::image::Format::R8G8B8A8 => {
                            mat.roughness_texture = Some(Texture::from_data_rgba8(
                                image.width as usize,
                                image.height as usize,
                                &image.pixels,
                                device,
                                queue,
                                Some("GLTF Mesh Roughness Texture"),
                                false,
                            ));
                        }
                        _ => {
                            todo!("Unsupported GLTF Texture Format");
                        }
                    }
                } else {
                    log::warn!("GLTF Mesh has no roughness texture");
                }
                if let Some(ao_texture) = material.occlusion_texture() {
                    let image = images.get(ao_texture.texture().source().index()).unwrap();
                    match image.format {
                        gltf::image::Format::R8G8B8 => {
                            mat.ambient_occlusion_texture = Some(Texture::from_data_r8g8b8(
                                image.width as usize,
                                image.height as usize,
                                &image.pixels,
                                device,
                                queue,
                                Some("GLTF Mesh Ambient Occlusion Texture"),
                                false,
                            ));
                        }
                        gltf::image::Format::R8G8B8A8 => {
                            mat.ambient_occlusion_texture = Some(Texture::from_data_rgba8(
                                image.width as usize,
                                image.height as usize,
                                &image.pixels,
                                device,
                                queue,
                                Some("GLTF Mesh Ambient Occlusion Texture"),
                                false,
                            ));
                        }
                        _ => {
                            todo!("Unsupported GLTF Texture Format");
                        }
                    }
                } else {
                    log::warn!("GLTF Mesh has no ambient occlusion texture");
                }

                mat.roughness = material.pbr_metallic_roughness().roughness_factor();
                mat.metallic = material.pbr_metallic_roughness().metallic_factor();
            }
        }

        calculate_tangents(&mut vertices, &indices);

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

        let transform = Transform::new();

        Ok(Self {
            mesh: Mesh {
                vertex_buffer,
                index_buffer,
                num_indices,
            },
            transform,
            material: mat,
        })
    }

    pub fn load_obj(
        path: impl AsRef<Path>,
        renderer: &Renderer,
        use_material: bool,
    ) -> anyhow::Result<Self> {
        let path = path;
        let device = &renderer.device;
        let queue = &renderer.queue;

        let (models, materials) = tobj::load_obj(
            path.as_ref(),
            &tobj::LoadOptions {
                single_index: true,
                triangulate: true,
                ..Default::default()
            },
        )?;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for model in models {
            let mesh = model.mesh;

            for i in 0..mesh.positions.len() / 3 {
                let position = [
                    mesh.positions[i * 3],
                    mesh.positions[i * 3 + 1],
                    mesh.positions[i * 3 + 2],
                ];
                let normal = [
                    mesh.normals[i * 3],
                    mesh.normals[i * 3 + 1],
                    mesh.normals[i * 3 + 2],
                ];
                let uv = [mesh.texcoords[i * 2], mesh.texcoords[i * 2 + 1]];

                vertices.push(Vertex {
                    position: glam::Vec3::from(position),
                    normal: glam::Vec3::from(normal),
                    uv: glam::Vec2::from(uv),
                    binormal: glam::Vec3::ZERO,
                    tangent: glam::Vec3::ZERO,
                    bitangent: glam::Vec3::ZERO,
                });
            }

            for index in mesh.indices {
                indices.push(index);
            }
        }

        let mut mat = Material::default();

        if use_material {
            if let Some(material) = materials?.into_iter().next() {
                if let Some(path) = material.diffuse_texture {
                    mat.diffuse_texture = Some(Texture::load(
                        path,
                        device,
                        queue,
                        Some("OBJ Mesh Diffuse Texture"),
                        false,
                    ));
                } else {
                    log::warn!("OBJ Mesh has no diffuse texture")
                }
                if let Some(path) = material.normal_texture {
                    mat.normal_texture = Some(Texture::load(
                        path,
                        device,
                        queue,
                        Some("OBJ Mesh Normal Texture"),
                        true,
                    ));
                } else {
                    log::warn!("OBJ Mesh has no normal texture")
                }
                if let Some(path) = material.specular_texture {
                    mat.roughness_texture = Some(Texture::load(
                        path,
                        device,
                        queue,
                        Some("OBJ Mesh Specular Texture"),
                        false,
                    ));
                } else {
                    log::warn!("OBJ Mesh has no specular texture")
                }

                if let Some(shininess) = material.shininess {
                    mat.roughness = 1.0 - shininess;
                    mat.metallic = 1.0;
                }
            }
        }

        calculate_tangents(&mut vertices, &indices);

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
            mesh: Mesh {
                vertex_buffer,
                index_buffer,
                num_indices,
            },
            transform: Transform::new(),
            material: mat,
        })
    }
}
