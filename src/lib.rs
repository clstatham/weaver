pub use weaver_core as core;

pub mod prelude {
    pub use anyhow::{anyhow, bail, ensure, Error, Result};
    pub use egui;
    pub use glam::*;
    pub use parking_lot::*;
    pub use weaver_core::prelude::*;
}
