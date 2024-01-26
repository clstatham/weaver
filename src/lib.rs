pub use weaver_core as core;
pub use weaver_ecs as ecs;

pub mod prelude {
    pub use anyhow;
    pub use egui;
    pub use glam::*;
    pub use parking_lot;
    pub use weaver_core::prelude::*;
    pub use weaver_ecs::prelude::*;
    pub use weaver_proc_macro::{Bundle, Component};
    pub use winit::event::MouseButton;
}
