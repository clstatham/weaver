use crate::prelude::*;

#[allow(dead_code)]
pub enum Materials {
    Wood,
    Metal,
    WoodTile,
    BrickWall,
    StoneWall,
    Banana,
}

impl Materials {
    pub fn load(
        &self,
        asset_server: &mut AssetServer,
        texture_scaling: f32,
    ) -> anyhow::Result<Material> {
        match self {
            // Wood_025
            Materials::Wood => {
                let base_color =
                    asset_server.load_texture("materials/Wood_025_SD/Wood_025_basecolor.jpg")?;
                let normal =
                    asset_server.load_texture("materials/Wood_025_SD/Wood_025_normal.jpg")?;
                let roughness =
                    asset_server.load_texture("materials/Wood_025_SD/Wood_025_roughness.jpg")?;
                let ao = asset_server
                    .load_texture("materials/Wood_025_SD/Wood_025_ambientOcclusion.jpg")?;
                Ok(asset_server.create_material(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                    Some(0.0),
                    Some(0.0),
                    Some(texture_scaling),
                ))
            }
            // Metal_006
            Materials::Metal => {
                let base_color =
                    asset_server.load_texture("materials/Metal_006_SD/Metal_006_basecolor.jpg")?;
                let normal =
                    asset_server.load_texture("materials/Metal_006_SD/Metal_006_normal.jpg")?;
                let roughness =
                    asset_server.load_texture("materials/Metal_006_SD/Metal_006_roughness.jpg")?;
                let ao = asset_server
                    .load_texture("materials/Metal_006_SD/Metal_006_ambientOcclusion.jpg")?;
                Ok(asset_server.create_material(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                    Some(1.0),
                    Some(1.0),
                    Some(texture_scaling),
                ))
            }
            // Wood_Herringbone_Tiles_004
            Materials::WoodTile => {
                let base_color = asset_server.load_texture(
                    "materials/Wood_Herringbone_Tiles_004_SD/Substance_Graph_BaseColor.jpg",
                )?;
                let normal = asset_server.load_texture(
                    "materials/Wood_Herringbone_Tiles_004_SD/Substance_Graph_Normal.jpg",
                )?;
                let roughness = asset_server.load_texture(
                    "materials/Wood_Herringbone_Tiles_004_SD/Substance_Graph_Roughness.jpg",
                )?;
                let ao = asset_server.load_texture(
                    "materials/Wood_Herringbone_Tiles_004_SD/Substance_Graph_AmbientOcclusion.jpg",
                )?;
                Ok(asset_server.create_material(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                    Some(0.0),
                    Some(0.5),
                    Some(texture_scaling),
                ))
            }
            // Brick_Wall_017
            Materials::BrickWall => {
                let base_color = asset_server
                    .load_texture("materials/Brick_Wall_017_SD/Brick_Wall_017_basecolor.jpg")?;
                let normal = asset_server
                    .load_texture("materials/Brick_Wall_017_SD/Brick_Wall_017_normal.jpg")?;
                let roughness = asset_server
                    .load_texture("materials/Brick_Wall_017_SD/Brick_Wall_017_roughness.jpg")?;
                let ao = asset_server.load_texture(
                    "materials/Brick_Wall_017_SD/Brick_Wall_017_ambientOcclusion.jpg",
                )?;
                Ok(asset_server.create_material(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                    Some(0.0),
                    Some(1.0),
                    Some(texture_scaling),
                ))
            }
            // Wall_Stone_021
            Materials::StoneWall => {
                let base_color = asset_server
                    .load_texture("materials/Wall_Stone_021_SD/Substance_graph_BaseColor.jpg")?;
                let normal = asset_server
                    .load_texture("materials/Wall_Stone_021_SD/Substance_graph_Normal.jpg")?;
                let roughness = asset_server
                    .load_texture("materials/Wall_Stone_021_SD/Substance_graph_Roughness.jpg")?;
                let ao = asset_server.load_texture(
                    "materials/Wall_Stone_021_SD/Substance_graph_AmbientOcclusion.jpg",
                )?;
                Ok(asset_server.create_material(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                    Some(0.0),
                    Some(1.0),
                    Some(texture_scaling),
                ))
            }
            // Food_0003
            Materials::Banana => {
                let base_color =
                    asset_server.load_texture("materials/Food_0003/food_0003_color_1k.jpg")?;
                let normal = asset_server
                    .load_texture("materials/Food_0003/food_0003_normal_opengl_1k.png")?;
                let roughness =
                    asset_server.load_texture("materials/Food_0003/food_0003_roughness_1k.jpg")?;
                let ao = asset_server.load_texture("materials/Food_0003/food_0003_ao_1k.jpg")?;
                Ok(asset_server.create_material(
                    Some(base_color),
                    Some(normal),
                    Some(roughness),
                    Some(ao),
                    Some(0.0),
                    Some(0.0),
                    Some(texture_scaling),
                ))
            }
        }
    }
}
