use camera::PbrCameraPlugin;
use material::MaterialPlugin;
use weaver_app::prelude::*;
use weaver_util::prelude::*;

pub mod camera;
pub mod material;
pub mod render;

pub mod prelude {
    pub use crate::PbrPlugin;
}

pub struct PbrPlugin;

impl Plugin for PbrPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(MaterialPlugin)?;
        app.add_plugin(PbrCameraPlugin)?;

        Ok(())
    }
}
