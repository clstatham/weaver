use fabricate::prelude::Component;

#[derive(Component)]
pub struct UpdateTime {
    start_time: std::time::Instant,
    last_update_time: std::time::Instant,
    pub delta_seconds: f32,
    pub total_seconds: f32,
}

impl UpdateTime {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            last_update_time: std::time::Instant::now(),
            delta_seconds: 0.0,
            total_seconds: 0.0,
        }
    }

    pub fn update(&mut self) {
        let now = std::time::Instant::now();
        self.delta_seconds = now.duration_since(self.last_update_time).as_secs_f32();
        self.total_seconds = now.duration_since(self.start_time).as_secs_f32();
        self.last_update_time = now;
    }
}

#[derive(Component)]
pub struct RenderTime {
    start_time: std::time::Instant,
    last_update_time: std::time::Instant,
    pub delta_seconds: f32,
    pub total_seconds: f32,
}

impl RenderTime {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            last_update_time: std::time::Instant::now(),
            delta_seconds: 0.0,
            total_seconds: 0.0,
        }
    }

    pub fn update(&mut self) {
        let now = std::time::Instant::now();
        self.delta_seconds = now.duration_since(self.last_update_time).as_secs_f32();
        self.total_seconds = now.duration_since(self.start_time).as_secs_f32();
        self.last_update_time = now;
    }
}
