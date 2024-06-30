use weaver_app::{plugin::Plugin, App};
use weaver_asset::AddAsset;
use weaver_util::prelude::Result;

pub mod generator;
pub mod loader;
pub mod parser;

pub struct BspPlugin;

impl Plugin for BspPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_asset_loader::<loader::Bsp, loader::BspLoader>();
        Ok(())
    }
}
