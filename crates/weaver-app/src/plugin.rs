use weaver_util::prelude::{impl_downcast, DowncastSync, Result};

use crate::App;

pub trait Plugin: DowncastSync {
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }

    fn build(&self, app: &mut App) -> Result<()>;

    #[allow(unused_variables)]
    fn finish(&self, app: &mut App) -> Result<()> {
        Ok(())
    }

    #[allow(unused_variables)]
    fn ready(&self, app: &App) -> bool {
        true
    }
}
impl_downcast!(Plugin);
