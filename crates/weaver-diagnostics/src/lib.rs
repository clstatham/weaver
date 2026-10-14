//! Frame time tracking and reporting.
//!
//! [`frame_time::FrameTimePlugin`] measures how long each frame takes and keeps the latest
//! numbers in a [`frame_time::FrameTime`] resource. On top of that,
//! [`frame_time::LogFrameTimePlugin`] logs summary statistics (mean, min, max, p99, FPS) at a fixed
//! interval, and with the `egui` feature [`egui_window::DiagnosticsWindowPlugin`] shows the same
//! statistics plus a graph in an egui window. Both reporting plugins add `FrameTimePlugin`
//! themselves if it isn't there yet.

/// An egui window with frame time statistics and a graph (feature `egui`).
#[cfg(feature = "egui")]
pub mod egui_window;
/// Frame time measurement, statistics and logging.
pub mod frame_time;

/// The reporting plugins.
pub mod prelude {
    #[cfg(feature = "egui")]
    pub use crate::egui_window::DiagnosticsWindowPlugin;
    pub use crate::frame_time::LogFrameTimePlugin;
}
