use weaver_app::{plugin::Plugin, App};
use weaver_ecs::{
    prelude::Resource,
    system::{ResMut, SystemStage},
};
use weaver_util::prelude::Result;

#[derive(Resource)]
pub struct Time {
    pub delta_time: f32,
    pub total_time: f32,
    pub frame_count: u32,
    last_update: std::time::Instant,
}

impl Time {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            delta_time: 0.0,
            total_time: 0.0,
            frame_count: 0,
            last_update: std::time::Instant::now(),
        }
    }

    pub fn update(&mut self) {
        let now = std::time::Instant::now();
        self.delta_time = now.duration_since(self.last_update).as_secs_f32();
        self.total_time += self.delta_time;
        self.frame_count += 1;
        self.last_update = now;
    }
}

pub struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.world().insert_resource(Time::new());
        app.add_system(update_time, SystemStage::PreUpdate)?;
        Ok(())
    }
}

fn update_time(mut time: ResMut<Time>) -> Result<()> {
    time.update();
    Ok(())
}
