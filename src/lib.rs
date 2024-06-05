pub use weaver_core as core;

pub mod prelude {
    pub use anyhow;
    pub use egui;
    pub use glam::*;
    pub use parking_lot::*;
    pub use weaver_core::prelude::*;
}
