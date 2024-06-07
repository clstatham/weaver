use material::MaterialLoader;
use weaver_app::prelude::*;
use weaver_asset::loader::AssetLoader;
use weaver_util::prelude::*;

pub mod material;

pub mod prelude {
    pub use crate::PbrPlugin;
}

pub struct PbrPlugin;

impl Plugin for PbrPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.get_resource_mut::<AssetLoader>()
            .unwrap()
            .add_loader(MaterialLoader);
        Ok(())
    }
}
