use std::any::Any;

use crate::App;

pub trait Plugin: Any {
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }

    fn build(&self, app: &mut App) -> anyhow::Result<()>;

    #[allow(unused_variables)]
    fn finish(&self, app: &mut App) -> anyhow::Result<()> {
        Ok(())
    }
}
