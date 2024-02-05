pub use weaver_core as core;

pub mod prelude {
    pub use anyhow;
    pub use egui;
    pub use fabricate::prelude::*;
    pub use glam::*;
    pub use parking_lot::*;
    pub use weaver_core::prelude::*;
    pub use weaver_proc_macro::{Bundle, Component};
    pub use winit::event::MouseButton;
}
