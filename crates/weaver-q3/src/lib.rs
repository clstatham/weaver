use weaver_app::{plugin::Plugin, App};
use weaver_util::Result;

pub mod bsp;
pub mod pk3;
pub mod shader;

pub struct Q3Plugin;

impl Plugin for Q3Plugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(shader::ShaderPlugin)?;
        app.add_plugin(bsp::BspPlugin)?;
        Ok(())
    }
}
