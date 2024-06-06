use std::any::Any;

use crate::App;

pub trait Plugin: Any {
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
    fn build(&self, app: &mut App) -> anyhow::Result<()>;
}
