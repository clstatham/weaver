use camera::PbrCameraPlugin;
use light::PointLightPlugin;
use material::MaterialPlugin;
use weaver_app::prelude::*;
use weaver_util::prelude::*;

pub mod camera;
pub mod light;
pub mod material;
pub mod render;

pub mod prelude {
    pub use crate::camera::*;
    pub use crate::light::*;
    pub use crate::material::*;
    pub use crate::PbrPlugin;
}

pub struct PbrPlugin;

impl Plugin for PbrPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(MaterialPlugin)?;
        app.add_plugin(PbrCameraPlugin)?;
        app.add_plugin(PointLightPlugin)?;

        Ok(())
    }
}
