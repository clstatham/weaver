use weaver_util::prelude::{impl_downcast, Downcast};

use crate::App;

pub trait Plugin: Downcast {
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }

    fn build(&self, app: &mut App) -> anyhow::Result<()>;

    #[allow(unused_variables)]
    fn finish(&self, app: &mut App) -> anyhow::Result<()> {
        Ok(())
    }
}
impl_downcast!(Plugin);
