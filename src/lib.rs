pub mod prelude {
    pub use anyhow;
    pub use egui;
    pub use glam::*;
    pub use parking_lot;
    pub use weaver_core::{self, prelude::*};
    pub use weaver_proc_macro::{Bundle, Component};
    pub use winit::event::MouseButton;
}
