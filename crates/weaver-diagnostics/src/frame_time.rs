use weaver_app::{plugin::Plugin, App, PreUpdate};
use weaver_ecs::component::{Res, ResMut};
use weaver_util::Result;

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
        app.add_system(update_frame_time, PreUpdate);

        Ok(())
    }
}

pub struct FrameTimeLogger {
    pub log_interval: std::time::Duration,
    pub last_log: std::time::Instant,
}

pub struct LogFrameTimePlugin {
    pub log_interval: std::time::Duration,
}

impl Plugin for LogFrameTimePlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(FrameTimePlugin)?;
        app.insert_resource(FrameTimeLogger {
            log_interval: self.log_interval,
            last_log: std::time::Instant::now(),
        });
        app.add_system_after(log_frame_time, update_frame_time, PreUpdate);

        Ok(())
    }
}

async fn update_frame_time(mut frame_time: ResMut<FrameTime>) {
    let now = std::time::Instant::now();
    frame_time.frame_time = now.duration_since(frame_time.last_update).as_secs_f32();
    frame_time.fps = 1.0 / frame_time.frame_time;
    frame_time.last_update = now;
    frame_time.frame_count += 1;
}

async fn log_frame_time(frame_time: Res<FrameTime>, mut logger: ResMut<FrameTimeLogger>) {
    let now = std::time::Instant::now();
    if now.duration_since(logger.last_log) >= logger.log_interval {
        log::info!(
            "Frame time: {:.4}ms, FPS: {:.2}",
            frame_time.frame_time * 1000.0,
            frame_time.fps
        );
        logger.last_log = now;
    }
}
