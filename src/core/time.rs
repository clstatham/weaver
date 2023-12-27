use weaver_proc_macro::Resource;

#[derive(Resource)]
pub struct Time {
    start_time: std::time::Instant,
    last_update_time: std::time::Instant,
    pub delta_time: f32,
    pub total_time: f32,
}

impl Time {
    pub fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            last_update_time: std::time::Instant::now(),
            delta_time: 0.0,
            total_time: 0.0,
        }
    }

    pub fn update(&mut self) {
        let now = std::time::Instant::now();
        self.delta_time = now.duration_since(self.last_update_time).as_secs_f32();
        self.total_time = now.duration_since(self.start_time).as_secs_f32();
        self.last_update_time = now;
    }
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}
