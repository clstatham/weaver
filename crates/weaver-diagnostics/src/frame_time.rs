use weaver_app::{plugin::Plugin, App};
use weaver_ecs::{prelude::Component, system::SystemStage, world::World};
use weaver_util::prelude::Result;

#[derive(Component)]
pub struct FrameTimeDiagnostics {
    pub frame_time: f32,
    pub fps: f32,
    pub last_update: std::time::Instant,
    pub frame_count: u32,
    pub log_interval: std::time::Duration,
    pub last_log: std::time::Instant,
}

pub struct LogFrameTimePlugin {
    pub log_interval: std::time::Duration,
}

impl Plugin for LogFrameTimePlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.world().insert_resource(FrameTimeDiagnostics {
            frame_time: 0.0,
            fps: 0.0,
            last_update: std::time::Instant::now(),
            frame_count: 0,
            log_interval: self.log_interval,
            last_log: std::time::Instant::now(),
        });
        app.add_system(log_frame_time, SystemStage::PreUpdate)?;

        Ok(())
    }
}

fn log_frame_time(world: &World) -> Result<()> {
    let mut frame_time = world.get_resource_mut::<FrameTimeDiagnostics>().unwrap();
    let now = std::time::Instant::now();
    frame_time.frame_time = now.duration_since(frame_time.last_update).as_secs_f32();
    frame_time.fps = 1.0 / frame_time.frame_time;
    frame_time.last_update = now;
    frame_time.frame_count += 1;

    if now.duration_since(frame_time.last_log) >= frame_time.log_interval {
        frame_time.last_log = now;

        log::info!(
            "Frame time: {:.2}ms, FPS: {:.2}",
            frame_time.frame_time * 1000.0,
            frame_time.fps
        );
    }

    Ok(())
}
