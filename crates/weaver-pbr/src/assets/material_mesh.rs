use weaver_asset::{loading::LoadCtx, prelude::*};
use weaver_core::{
    mesh::{calculate_normals, calculate_tangents, Mesh, Vertex},
    prelude::{Vec2, Vec3, Vec4},
    texture::Texture,
};
use weaver_ecs::prelude::Resource;
use weaver_util::prelude::{anyhow, bail, Result};

use crate::prelude::Material;

#[derive(Asset)]
pub struct LoadedModelWithMaterials {
    pub primitives: Vec<LoadedMaterialMeshPrimitive>,
}

impl IntoIterator for LoadedModelWithMaterials {
    type Item = (Material, Mesh);
    type IntoIter = std::iter::Map<
        std::vec::IntoIter<LoadedMaterialMeshPrimitive>,
        fn(LoadedMaterialMeshPrimitive) -> (Material, Mesh),
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.primitives.into_iter().map(|m| m.into_parts())
    }
}

#[derive(Asset)]
pub struct LoadedMaterialMeshPrimitive {
    pub material: Material,
    pub mesh: Mesh,
}

impl LoadedMaterialMeshPrimitive {
    pub fn into_parts(self) -> (Material, Mesh) {
        (self.material, self.mesh)
    }
}

#[derive(Resource, Default)]
pub struct ObjMaterialModelLoader;

impl LoadAsset<LoadedModelWithMaterials> for ObjMaterialModelLoader {
    fn load(&self, ctx: &mut LoadCtx<'_, '_>) -> Result<LoadedModelWithMaterials> {
        load_obj_material_mesh(ctx)
    }
}

pub fn load_obj_material_mesh(ctx: &mut LoadCtx<'_, '_>) -> Result<LoadedModelWithMaterials> {
    let bytes = ctx.read_original()?;
    let (models, materials) = tobj::load_obj_buf(
        &mut std::io::Cursor::new(bytes),
        &tobj::LoadOptions {
            single_index: true,
            triangulate: true,
            ignore_points: true,
            ignore_lines: true,
        },
        |mtl_path| {
            let bytes = ctx
                .filesystem()
                .read_sub_path(mtl_path.as_os_str().to_str().unwrap())
                .map_err(|e| {
                    log::error!("Failed to open MTL file: {:?}", e);
                    tobj::LoadError::OpenFileFailed
                })?;
            tobj::load_mtl_buf(&mut std::io::Cursor::new(bytes))
        },
    )?;
    let materials = materials?;

    let mut primitives = Vec::with_capacity(models.len());

    let black_texture = Texture::from_rgba8(&[0, 0, 0, 255], 1, 1);
    let white_texture = Texture::from_rgba8(&[255, 255, 255, 255], 1, 1);

    for model in &models {
        let mesh = &model.mesh;

        let mut vertices = Vec::with_capacity(mesh.positions.len() / 3);
        let mut indices = Vec::with_capacity(mesh.indices.len());
        let has_normals = !mesh.normals.is_empty();

        for i in 0..mesh.positions.len() / 3 {
            let position = [
                mesh.positions[i * 3],
                mesh.positions[i * 3 + 1],
                mesh.positions[i * 3 + 2],
            ];
            let normal = if has_normals {
                [
                    mesh.normals[i * 3],
                    mesh.normals[i * 3 + 1],
                    mesh.normals[i * 3 + 2],
                ]
            } else {
                [0.0, 0.0, 0.0]
            };
            let uv = [mesh.texcoords[i * 2], 1.0 - mesh.texcoords[i * 2 + 1]];

            vertices.push(Vertex {
                position: Vec3::from(position),
                normal: Vec3::from(normal).normalize(),
                tex_coords: Vec2::from(uv),
                tangent: Vec3::ZERO,
            });
        }

        for index in &mesh.indices {
            indices.push(*index);
        }

        if !has_normals {
            calculate_normals(&mut vertices, &indices);
        }

        calculate_tangents(&mut vertices, &indices);

        let material = materials.get(model.mesh.material_id.unwrap_or(0));

        match material {
            Some(material) => {
                let diffuse = material.diffuse.unwrap_or([1.0, 1.0, 1.0]);
                let diffuse_texture = match &material.diffuse_texture {
                    Some(texture) => {
                        let bytes = ctx.filesystem().read_sub_path(texture)?;
                        let rgba8 = image::load_from_memory(&bytes)?.to_rgba8();
                        Texture::from_rgba8(&rgba8, rgba8.width(), rgba8.height())
                    }
                    None => {
                        #[cfg(debug_assertions)]
                        log::warn!("Material does not have a diffuse texture");
                        white_texture.clone()
                    }
                };
                let normal_texture = match &material.normal_texture {
                    Some(texture) => {
                        let bytes = ctx.filesystem().read_sub_path(texture)?;
                        let rgba8 = image::load_from_memory(&bytes)?.to_rgba8();
                        Texture::from_rgba8(&rgba8, rgba8.width(), rgba8.height())
                    }
                    None => {
                        #[cfg(debug_assertions)]
                        log::warn!("Material does not have a normal texture");
                        black_texture.clone()
                    }
                };
                let ao = material.ambient.unwrap_or([1.0, 1.0, 1.0]);
                let ao_texture = match &material.ambient_texture {
                    Some(texture) => {
                        let bytes = ctx.filesystem().read_sub_path(texture)?;
                        let rgba8 = image::load_from_memory(&bytes)?.to_rgba8();
                        Texture::from_rgba8(&rgba8, rgba8.width(), rgba8.height())
                    }
                    None => {
                        #[cfg(debug_assertions)]
                        log::warn!("Material does not have an AO texture");
                        white_texture.clone()
                    }
                };

                let metallic = material.shininess.unwrap_or(0.0);
                let metallic_roughness_texture = match &material.shininess_texture {
                    Some(texture) => {
                        let bytes = ctx.filesystem().read_sub_path(texture)?;
                        let rgba8 = image::load_from_memory(&bytes)?.to_rgba8();
                        Texture::from_rgba8(&rgba8, rgba8.width(), rgba8.height())
                    }
                    None => {
                        #[cfg(debug_assertions)]
                        log::warn!("Material does not have a metallic roughness texture");
                        black_texture.clone()
                    }
                };

                let mut textures = ctx.get_resource_mut::<Assets<Texture>>().unwrap();

                let material = Material {
                    diffuse: diffuse.into(),
                    diffuse_texture: textures.insert(diffuse_texture),
                    normal_texture: textures.insert(normal_texture),
                    metallic: metallic / 100.0,
                    roughness: 0.0,
                    metallic_roughness_texture: textures.insert(metallic_roughness_texture),
                    ao: ao.into_iter().sum::<f32>() / 3.0,
                    ao_texture: textures.insert(ao_texture),
                    texture_scale: 1.0,
                };

                primitives.push(LoadedMaterialMeshPrimitive {
                    material,
                    mesh: Mesh::new(vertices, indices),
                });
            }
            None => {
                log::warn!("Model does not have a material");

                let mut textures = ctx.get_resource_mut::<Assets<Texture>>().unwrap();

                let material = Material {
                    diffuse: [1.0, 1.0, 1.0].into(),
                    diffuse_texture: textures.insert(white_texture.clone()),
                    normal_texture: textures.insert(black_texture.clone()),
                    metallic: 0.0,
                    roughness: 0.0,
                    metallic_roughness_texture: textures.insert(black_texture.clone()),
                    ao: 1.0,
                    ao_texture: textures.insert(white_texture.clone()),
                    texture_scale: 1.0,
                };

                primitives.push(LoadedMaterialMeshPrimitive {
                    material,
                    mesh: Mesh::new(vertices, indices),
                });
            }
        }
    }

    Ok(LoadedModelWithMaterials { primitives })
}

#[derive(Resource, Default)]
pub struct GltfMaterialModelLoader;

impl LoadAsset<LoadedModelWithMaterials> for GltfMaterialModelLoader {
    fn load(&self, ctx: &mut LoadCtx<'_, '_>) -> Result<LoadedModelWithMaterials> {
        load_gltf_material_mesh(ctx)
    }
}

pub fn load_gltf_material_mesh(ctx: &mut LoadCtx<'_, '_>) -> Result<LoadedModelWithMaterials> {
    let bytes = ctx.read_original()?;
    let (document, buffers, images) = gltf::import_slice(bytes)?;

    let mut primitives = Vec::new();

    let mut textures = ctx.get_resource_mut::<Assets<Texture>>().unwrap();

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let material = load_material(primitive.material(), &images, &mut textures)?;

            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let mut positions = reader.read_positions().ok_or_else(|| {
                anyhow!("mesh primitive does not have positions: {:?}", primitive)
            })?;

            let mut normals = reader
                .read_normals()
                .ok_or_else(|| anyhow!("mesh primitive does not have normals: {:?}", primitive))?;

            let mut tex_coords = reader
                .read_tex_coords(0)
                .ok_or_else(|| anyhow!("mesh primitive does not have tex coords: {:?}", primitive))?
                .into_f32();

            let mut tangents = reader
                .read_tangents()
                .ok_or_else(|| anyhow!("mesh primitive does not have tangents: {:?}", primitive))?;

            let indices_iter = reader
                .read_indices()
                .ok_or_else(|| anyhow!("mesh primitive does not have indices: {:?}", primitive))?
                .into_u32();

            let vertices_iter = positions
                .by_ref()
                .zip(normals.by_ref())
                .zip(tex_coords.by_ref())
                .zip(tangents.by_ref())
                .map(|(((position, normal), tex_coord), tangent)| Vertex {
                    position: Vec3::from(position),
                    normal: Vec3::from(normal),
                    tex_coords: Vec2::from(tex_coord),
                    tangent: Vec4::from(tangent).truncate(),
                });

            let vertices = vertices_iter.collect();
            let indices = indices_iter.collect();

            primitives.push(LoadedMaterialMeshPrimitive {
                material,
                mesh: Mesh::new(vertices, indices),
            });
        }
    }

    Ok(LoadedModelWithMaterials { primitives })
}

fn load_material(
    material: gltf::Material,
    images: &[gltf::image::Data],
    textures: &mut Assets<Texture>,
) -> Result<Material> {
    let metallic = material.pbr_metallic_roughness().metallic_factor();
    let roughness = material.pbr_metallic_roughness().roughness_factor();
    let ao = material
        .occlusion_texture()
        .map_or(1.0, |info| info.strength());
    let diffuse = material.pbr_metallic_roughness().base_color_factor();
    let diffuse_texture = material
        .pbr_metallic_roughness()
        .base_color_texture()
        .map(|info| images[info.texture().index()].clone());
    let normal_texture = material
        .normal_texture()
        .map(|info| images[info.texture().index()].clone());
    let metallic_roughness_texture = material
        .pbr_metallic_roughness()
        .metallic_roughness_texture()
        .map(|info| images[info.texture().index()].clone());
    let ao_texture = material
        .occlusion_texture()
        .map(|info| images[info.texture().index()].clone());

    let diffuse_texture =
        diffuse_texture.ok_or_else(|| anyhow!("Material must have a diffuse texture"))?;
    let normal_texture =
        normal_texture.ok_or_else(|| anyhow!("Material must have a normal texture"))?;
    let metallic_roughness_texture = metallic_roughness_texture
        .ok_or_else(|| anyhow!("Material must have a metallic roughness texture"))?;
    let ao_texture = ao_texture.ok_or_else(|| anyhow!("Material must have an AO texture"))?;

    let diffuse_texture = match diffuse_texture.format {
        gltf::image::Format::R8G8B8 => Texture::from_rgb8(
            &diffuse_texture.pixels,
            diffuse_texture.width,
            diffuse_texture.height,
        ),
        gltf::image::Format::R8G8B8A8 => Texture::from_rgba8(
            &diffuse_texture.pixels,
            diffuse_texture.width,
            diffuse_texture.height,
        ),
        format => bail!(
            "Diffuse texture must be in RGB8 or RGBA8 format (got {:?})",
            format
        ),
    };
    let normal_texture = match normal_texture.format {
        gltf::image::Format::R8G8B8 => Texture::from_rgb8(
            &normal_texture.pixels,
            normal_texture.width,
            normal_texture.height,
        ),
        gltf::image::Format::R8G8B8A8 => Texture::from_rgba8(
            &normal_texture.pixels,
            normal_texture.width,
            normal_texture.height,
        ),
        format => bail!(
            "Normal texture must be in RGB8 or RGBA8 format (got {:?})",
            format
        ),
    };
    let metallic_roughness_texture = match metallic_roughness_texture.format {
        gltf::image::Format::R8G8B8 => Texture::from_rgb8(
            &metallic_roughness_texture.pixels,
            metallic_roughness_texture.width,
            metallic_roughness_texture.height,
        ),
        gltf::image::Format::R8G8B8A8 => Texture::from_rgba8(
            &metallic_roughness_texture.pixels,
            metallic_roughness_texture.width,
            metallic_roughness_texture.height,
        ),
        format => bail!(
            "Metallic/Roughness texture must be in RGB8 or RGBA8 format (got {:?})",
            format
        ),
    };
    let ao_texture = match ao_texture.format {
        gltf::image::Format::R8G8B8 => {
            Texture::from_rgb8(&ao_texture.pixels, ao_texture.width, ao_texture.height)
        }
        gltf::image::Format::R8G8B8A8 => {
            Texture::from_rgba8(&ao_texture.pixels, ao_texture.width, ao_texture.height)
        }
        format => bail!(
            "Ambient Occlusion texture must be in RGB8 or RGBA8 format (got {:?})",
            format
        ),
    };

    let material = Material {
        diffuse: diffuse.into(),
        diffuse_texture: textures.insert(diffuse_texture),
        normal_texture: textures.insert(normal_texture),
        metallic,
        roughness,
        metallic_roughness_texture: textures.insert(metallic_roughness_texture),
        ao,
        ao_texture: textures.insert(ao_texture),
        texture_scale: 1.0,
    };

    Ok(material)
}
