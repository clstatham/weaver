pub use weaver_core as core;
pub use weaver_ecs as ecs;
pub use weaver_util as util;

pub mod prelude {
    pub use anyhow::{anyhow, bail, ensure, Error, Result};
    pub use egui;
    pub use glam::*;
    pub use parking_lot::*;
    pub use weaver_core::prelude::*;
    pub use weaver_ecs::prelude::*;
    pub use weaver_util::prelude::*;
}
