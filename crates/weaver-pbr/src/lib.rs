use weaver_app::prelude::*;
use weaver_util::prelude::*;

pub mod material;

pub mod prelude {
    pub use crate::PbrPlugin;
}

pub struct PbrPlugin;

impl Plugin for PbrPlugin {
    fn build(&self, _app: &mut App) -> Result<()> {
        Ok(())
    }
}
