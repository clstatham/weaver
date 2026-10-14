//! The egui diagnostics window.

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use egui::{Color32, Ui};
use egui_plot::{Corner, FilledArea, Legend, Line, Plot};
use weaver_app::{App, AppStage, plugin::Plugin};
use weaver_ecs::prelude::*;
use weaver_egui::prelude::*;
use weaver_util::prelude::*;

use crate::frame_time::{FrameTime, FrameTimePlugin, FrameTimeStats};

/// How far back the frame time graph goes.
const GRAPH_WINDOW: Duration = Duration::from_secs(2);
/// How many time slices the graph groups frames into. At high frame rates there are thousands of
/// frames in the window, and plotting each one costs a noticeable share of the frame being measured.
const GRAPH_BUCKETS: usize = 200;
const GRAPH_SIZE: [f32; 2] = [320.0, 140.0];
const FRAME_TIME_COLOR: Color32 = Color32::from_rgb(110, 200, 120);

/// Shows an egui window with frame time statistics (the same ones [`crate::frame_time::LogFrameTimePlugin`]
/// logs) and a graph of recent frame times. Requires [`EguiPlugin`].
pub struct DiagnosticsWindowPlugin {
    /// How often the statistics are recomputed.
    pub stats_interval: Duration,
}

impl DiagnosticsWindowPlugin {
    /// Creates the plugin; the statistics are recomputed every `stats_interval`.
    pub fn new(stats_interval: Duration) -> Self {
        Self { stats_interval }
    }
}

impl Default for DiagnosticsWindowPlugin {
    fn default() -> Self {
        Self {
            stats_interval: Duration::from_millis(500),
        }
    }
}

impl Plugin for DiagnosticsWindowPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        // LogFrameTimePlugin may have added it already
        if !app.has_resource::<FrameTime>() {
            app.add_plugin(FrameTimePlugin)?;
        }
        app.insert_resource(DiagnosticsWindow::new(self.stats_interval));
        // the EguiContext only reaches the main world after the first render extract
        app.add_system(
            diagnostics_window.run_if(resource_exists::<EguiContext>()),
            AppStage::Update,
        );

        Ok(())
    }
}

/// State for the diagnostics window: recent frame times and the stats computed from them.
pub struct DiagnosticsWindow {
    stats_interval: Duration,
    last_stats_update: Instant,
    /// Frame times since the last stats update.
    interval_samples: Vec<f32>,
    stats: Option<FrameTimeStats>,
    /// Frame times covering the last [`GRAPH_WINDOW`], oldest first.
    history: VecDeque<f32>,
    /// Sum of `history`, in seconds.
    history_duration: f64,
    /// Top of the graph's y axis, in milliseconds. Follows the p99 of `history` rather than its max,
    /// so one hitch doesn't flatten the graph, and only changes with the stats, so it doesn't jitter.
    graph_ceiling_ms: f64,
    /// Scratch space for computing `graph_ceiling_ms`.
    scratch: Vec<f32>,
}

impl DiagnosticsWindow {
    fn new(stats_interval: Duration) -> Self {
        Self {
            stats_interval,
            last_stats_update: Instant::now(),
            interval_samples: Vec::new(),
            stats: None,
            history: VecDeque::new(),
            history_duration: 0.0,
            graph_ceiling_ms: 0.0,
            scratch: Vec::new(),
        }
    }

    fn record(&mut self, frame_time: f32) {
        self.interval_samples.push(frame_time);

        self.history.push_back(frame_time);
        self.history_duration += frame_time as f64;
        while self.history_duration > GRAPH_WINDOW.as_secs_f64() && self.history.len() > 1 {
            let oldest = self.history.pop_front().unwrap();
            self.history_duration -= oldest as f64;
        }

        let now = Instant::now();
        if now.duration_since(self.last_stats_update) >= self.stats_interval
            && let Some(stats) = FrameTimeStats::from_samples(&mut self.interval_samples)
        {
            self.stats = Some(stats);
            self.interval_samples.clear();
            self.last_stats_update = now;

            self.scratch.clear();
            self.scratch.extend(&self.history);
            if let Some(history_stats) = FrameTimeStats::from_samples(&mut self.scratch) {
                self.graph_ceiling_ms = nice_ceiling(history_stats.p99 * 1000.0 * 1.25) as f64;
            }
        }
    }

    fn ui(&self, ui: &mut Ui) {
        match &self.stats {
            Some(stats) => stats_grid(ui, stats),
            None => {
                ui.label("Collecting frame times...");
            }
        }
        ui.add_space(4.0);
        self.frame_time_graph(ui);
    }

    /// Plots the last [`GRAPH_WINDOW`] of frame times, grouped into [`GRAPH_BUCKETS`] time slices:
    /// a band from each slice's fastest to slowest frame, and a line through its mean.
    fn frame_time_graph(&self, ui: &mut Ui) {
        let window = GRAPH_WINDOW.as_secs_f64();
        let bucket_width = window / GRAPH_BUCKETS as f64;

        // (min, max, sum, count) per bucket, indexed by age: bucket 0 is the newest
        let mut buckets = [(f32::MAX, f32::MIN, 0.0f32, 0u32); GRAPH_BUCKETS];
        let mut age = 0.0f64;
        for &frame_time in self.history.iter().rev() {
            let Some(bucket) = buckets.get_mut((age / bucket_width) as usize) else {
                break;
            };
            bucket.0 = bucket.0.min(frame_time);
            bucket.1 = bucket.1.max(frame_time);
            bucket.2 += frame_time;
            bucket.3 += 1;
            age += frame_time as f64;
        }

        let mut xs = Vec::with_capacity(GRAPH_BUCKETS);
        let mut mins = Vec::with_capacity(GRAPH_BUCKETS);
        let mut maxs = Vec::with_capacity(GRAPH_BUCKETS);
        let mut means = Vec::with_capacity(GRAPH_BUCKETS);
        // oldest first, so x increases
        for (i, &(min, max, sum, count)) in buckets.iter().enumerate().rev() {
            if count == 0 {
                continue;
            }
            let x = -(i as f64 + 0.5) * bucket_width;
            xs.push(x);
            mins.push(min as f64 * 1000.0);
            maxs.push(max as f64 * 1000.0);
            means.push([x, (sum / count as f32) as f64 * 1000.0]);
        }

        let ceiling_ms = self.graph_ceiling_ms;
        Plot::new("frame_time_graph")
            .width(GRAPH_SIZE[0])
            .height(GRAPH_SIZE[1])
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .allow_double_click_reset(false)
            .legend(Legend::default().position(Corner::LeftTop))
            .x_axis_formatter(|mark, _| format!("{}s", mark.value))
            .y_axis_formatter(|mark, _| format!("{}ms", mark.value))
            .show(ui, |plot_ui| {
                plot_ui.set_plot_bounds_x(-window..=0.0);
                if ceiling_ms > 0.0 {
                    plot_ui.set_plot_bounds_y(0.0..=ceiling_ms);
                }
                plot_ui.add(
                    FilledArea::new("min–max", &xs, &mins, &maxs)
                        .fill_color(FRAME_TIME_COLOR.gamma_multiply(0.35)),
                );
                plot_ui.line(Line::new("mean", means).color(FRAME_TIME_COLOR));
            });
    }
}

fn stats_grid(ui: &mut Ui, stats: &FrameTimeStats) {
    egui::Grid::new("frame_time_stats")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            let mut row = |label: &str, value: String| {
                ui.label(label);
                ui.monospace(value);
                ui.end_row();
            };
            row("FPS", format!("{:.1}", stats.fps()));
            row("1% low FPS", format!("{:.1}", stats.low_fps()));
            row("Mean", format!("{:.3}ms", stats.mean * 1000.0));
            row("Min", format!("{:.3}ms", stats.min * 1000.0));
            row("p99", format!("{:.3}ms", stats.p99 * 1000.0));
            row("Max", format!("{:.3}ms", stats.max * 1000.0));
            row("Frames", stats.count.to_string());
        });
}

/// Rounds `value` up to 1, 2 or 5 times a power of ten, for a readable graph scale.
fn nice_ceiling(value: f32) -> f32 {
    let magnitude = 10.0f32.powf(value.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|step| step * magnitude)
        .find(|&ceiling| ceiling >= value)
        .unwrap_or(10.0 * magnitude)
}

/// Records this frame's time and draws the "Diagnostics" window. Runs in `Update` once the
/// [`EguiContext`] exists.
pub fn diagnostics_window(
    ctx: Res<EguiContext>,
    frame_time: Res<FrameTime>,
    mut window: ResMut<DiagnosticsWindow>,
) {
    // the first frame's time spans app startup, so it isn't a real frame time
    if frame_time.frame_count > 1 {
        window.record(frame_time.frame_time);
    }

    ctx.with_ctx(|ctx| {
        egui::Window::new("Diagnostics")
            .resizable(false)
            .show(ctx, |ui| window.ui(ui));
    });
}
