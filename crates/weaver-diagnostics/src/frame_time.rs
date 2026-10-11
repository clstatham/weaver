use std::time::Duration;

use weaver_app::{App, AppStage, plugin::Plugin};
use weaver_ecs::{
    component::{Res, ResMut},
    system::IntoSystemConfig,
};
use weaver_util::prelude::*;

pub struct FrameTime {
    pub frame_time: f32,
    pub fps: f32,
    pub last_update: std::time::Instant,
    pub frame_count: u32,
}

pub struct FrameTimePlugin;

impl Plugin for FrameTimePlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.insert_resource(FrameTime {
            frame_time: 0.0,
            fps: 0.0,
            last_update: std::time::Instant::now(),
            frame_count: 0,
        });
        app.add_system(update_frame_time, AppStage::PreUpdate);

        Ok(())
    }
}

pub struct FrameTimeLogger {
    pub log_interval: Duration,
    pub last_log: std::time::Instant,
    /// Frame times (in seconds) recorded since the last log.
    samples: Vec<f32>,
}

pub struct LogFrameTimePlugin {
    pub log_interval: Duration,
}

impl LogFrameTimePlugin {
    pub fn new(log_interval: Duration) -> Self {
        Self { log_interval }
    }
}

impl Default for LogFrameTimePlugin {
    fn default() -> Self {
        Self {
            log_interval: Duration::from_secs(1),
        }
    }
}

impl Plugin for LogFrameTimePlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        // DiagnosticsWindowPlugin may have added it already
        if !app.has_resource::<FrameTime>() {
            app.add_plugin(FrameTimePlugin)?;
        }
        app.insert_resource(FrameTimeLogger {
            log_interval: self.log_interval,
            last_log: std::time::Instant::now(),
            samples: Vec::new(),
        });
        app.add_system(log_frame_time.after(update_frame_time), AppStage::PreUpdate);

        Ok(())
    }
}

fn update_frame_time(mut frame_time: ResMut<FrameTime>) {
    let now = std::time::Instant::now();
    frame_time.frame_time = now.duration_since(frame_time.last_update).as_secs_f32();
    frame_time.fps = 1.0 / frame_time.frame_time;
    frame_time.last_update = now;
    frame_time.frame_count += 1;
}

/// Statistics over a set of frame times, all in seconds.
#[derive(Debug, Clone, Copy)]
pub struct FrameTimeStats {
    pub count: usize,
    pub mean: f32,
    pub min: f32,
    pub max: f32,
    pub p99: f32,
}

impl FrameTimeStats {
    /// Computes statistics over `samples` (frame times in seconds). Reorders `samples`.
    /// Returns `None` if there are no samples.
    pub fn from_samples(samples: &mut [f32]) -> Option<Self> {
        let count = samples.len();
        if count == 0 {
            return None;
        }
        let mean = samples.iter().sum::<f32>() / count as f32;
        let (min, max) = samples.iter().fold((f32::MAX, f32::MIN), |(min, max), &t| {
            (min.min(t), max.max(t))
        });
        let p99_index = ((count - 1) as f32 * 0.99).round() as usize;
        let (_, &mut p99, _) = samples.select_nth_unstable_by(p99_index, f32::total_cmp);
        Some(Self {
            count,
            mean,
            min,
            max,
            p99,
        })
    }

    /// Average frames per second.
    pub fn fps(&self) -> f32 {
        1.0 / self.mean
    }

    /// The "1% low" FPS: the frame rate of the 99th percentile frame time.
    pub fn low_fps(&self) -> f32 {
        1.0 / self.p99
    }
}

fn log_frame_time(frame_time: Res<FrameTime>, mut logger: ResMut<FrameTimeLogger>) {
    // the first frame's time spans app startup, so it isn't a real frame time
    if frame_time.frame_count > 1 {
        logger.samples.push(frame_time.frame_time);
    }

    let now = std::time::Instant::now();
    if now.duration_since(logger.last_log) < logger.log_interval {
        return;
    }
    let Some(stats) = FrameTimeStats::from_samples(&mut logger.samples) else {
        return;
    };

    log::info!(
        "Frame time: mean {:.3}ms, min {:.3}ms, p99 {:.3}ms, max {:.3}ms | FPS: {:.1} (1% low {:.1}) over {} frames",
        stats.mean * 1000.0,
        stats.min * 1000.0,
        stats.p99 * 1000.0,
        stats.max * 1000.0,
        stats.fps(),
        stats.low_fps(),
        stats.count,
    );

    logger.samples.clear();
    logger.last_log = now;
}
