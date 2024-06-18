use camera::PbrCameraPlugin;
use light::PointLightPlugin;
use material::MaterialPlugin;
use render::PbrNode;
use weaver_app::prelude::*;
use weaver_renderer::{pipeline::RenderPipelinePlugin, RenderApp};
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
        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.add_plugin(MaterialPlugin)?;
        render_app.add_plugin(PbrCameraPlugin)?;
        render_app.add_plugin(PointLightPlugin)?;
        render_app.add_plugin(RenderPipelinePlugin::<PbrNode>::default())?;

        Ok(())
    }
}
