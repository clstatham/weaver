use weaver_util::{impl_downcast, DowncastSync, Result};

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
}
impl_downcast!(Plugin);
