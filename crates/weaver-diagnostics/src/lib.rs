#[cfg(feature = "egui")]
pub mod egui_window;
pub mod frame_time;

pub mod prelude {
    #[cfg(feature = "egui")]
    pub use crate::egui_window::DiagnosticsWindowPlugin;
    pub use crate::frame_time::LogFrameTimePlugin;
}
