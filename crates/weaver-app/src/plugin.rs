use weaver_util::prelude::*;

use crate::App;

#[allow(unused_variables)]
pub trait Plugin: DowncastSync {
    fn type_name(&self) -> &str {
        std::any::type_name::<Self>()
    }

    fn build(&self, app: &mut App) -> Result<()>;

    fn finish(&self, app: &mut App) -> Result<()> {
        Ok(())
    }

    fn ready(&self, app: &App) -> bool {
        true
    }

    fn cleanup(&self, app: &mut App) -> Result<()> {
        Ok(())
    }
}
impl_downcast!(Plugin);

pub struct DummyPlugin;

impl Plugin for DummyPlugin {
    fn build(&self, _app: &mut App) -> Result<()> {
        Ok(())
    }
}
