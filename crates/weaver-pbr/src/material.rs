use std::path::Path;

use weaver_asset::{loader::LoadAsset, Assets, Handle, UntypedHandle};
use weaver_core::{color::Color, texture::Texture};
use weaver_ecs::prelude::{Entity, Query, World};
use weaver_renderer::{extract::RenderComponent, prelude::*, texture::GpuTexture};
use weaver_util::prelude::*;

pub struct Material {
    pub diffuse: Color,
    pub diffuse_texture: Handle<Texture>,

    pub normal_texture: Handle<Texture>,

    pub metallic: f32,
    pub roughness: f32,
    pub metallic_roughness_texture: Handle<Texture>,

    pub ao: f32,
    pub ao_texture: Handle<Texture>,
}

pub struct MaterialLoader;

impl LoadAsset for MaterialLoader {
    fn load_asset(&self, path: &Path, assets: &mut Assets) -> Result<UntypedHandle> {
        let (document, _buffers, images) = gltf::import(path)?;
        if document.materials().count() != 1 {
            bail!("Material file must contain exactly one material");
        }

        let material = document.materials().next().unwrap();
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
            _ => bail!("Diffuse texture must be in RGB8 or RGBA8 format"),
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
            _ => bail!("Normal texture must be in RGB8 or RGBA8 format"),
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
            _ => bail!("Metallic roughness texture must be in RGB8 or RGBA8 format"),
        };
        let ao_texture = match ao_texture.format {
            gltf::image::Format::R8G8B8 => {
                Texture::from_rgb8(&ao_texture.pixels, ao_texture.width, ao_texture.height)
            }
            gltf::image::Format::R8G8B8A8 => {
                Texture::from_rgba8(&ao_texture.pixels, ao_texture.width, ao_texture.height)
            }
            _ => bail!("AO texture must be in RGB8 or RGBA8 format"),
        };

        let material = Material {
            diffuse: diffuse.into(),
            diffuse_texture: assets.insert(diffuse_texture, None),
            normal_texture: assets.insert(normal_texture, None),
            metallic,
            roughness,
            metallic_roughness_texture: assets.insert(metallic_roughness_texture, None),
            ao,
            ao_texture: assets.insert(ao_texture, None),
        };

        Ok(assets.insert(material, Some(path)).into())
    }
}

pub struct GpuMaterial {
    pub diffuse: Color,
    pub diffuse_texture: GpuTexture,

    pub normal_texture: GpuTexture,

    pub metallic: f32,
    pub roughness: f32,
    pub metallic_roughness_texture: GpuTexture,

    pub ao: f32,
    pub ao_texture: GpuTexture,
}

impl RenderComponent for GpuMaterial {
    fn query() -> Query {
        Query::new().read::<Handle<Material>>()
    }

    fn extract_render_component(entity: Entity, world: &World) -> Option<Self>
    where
        Self: Sized,
    {
        let renderer = world.get_resource::<Renderer>()?;
        let assets = world.get_resource::<Assets>()?;
        let material = world.get_component::<Handle<Material>>(entity)?;

        let material = assets.get(*material)?;

        let diffuse_texture = assets.get(material.diffuse_texture)?;
        let diffuse_texture = GpuTexture::from_image(&renderer, diffuse_texture)?;

        let normal_texture = assets.get(material.normal_texture)?;
        let normal_texture = GpuTexture::from_image(&renderer, normal_texture)?;

        let metallic_roughness_texture = assets.get(material.metallic_roughness_texture)?;
        let metallic_roughness_texture =
            GpuTexture::from_image(&renderer, metallic_roughness_texture)?;

        let ao_texture = assets.get(material.ao_texture)?;
        let ao_texture = GpuTexture::from_image(&renderer, ao_texture)?;

        Some(Self {
            diffuse: material.diffuse,
            diffuse_texture,
            normal_texture,
            metallic: material.metallic,
            roughness: material.roughness,
            metallic_roughness_texture,
            ao: material.ao,
            ao_texture,
        })
    }
}
