use weaver::prelude::*;

pub struct FpsCounter {
    pub fps: f32,
    pub frame_count: u32,
    pub last_update: std::time::Instant,
}

impl FpsCounter {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            fps: 0.0,
            frame_count: 0,
            last_update: std::time::Instant::now(),
        }
    }

    pub fn update(&mut self) {
        self.frame_count += 1;
        let now = std::time::Instant::now();
        if now - self.last_update > std::time::Duration::from_secs(1) {
            self.last_update = now;
            self.fps = self.frame_count as f32;
            self.frame_count = 0;
        }
    }
}

pub struct FpsDisplay {
    last_frame: std::time::Instant,
    last_update: std::time::Instant,
    update_interval: std::time::Duration,
    last_print: std::time::Instant,
    print_interval: std::time::Duration,
    fps_buffer: Vec<f32>,
    fps: f32,
}

impl FpsDisplay {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            last_frame: std::time::Instant::now(),
            last_update: std::time::Instant::now(),
            last_print: std::time::Instant::now(),
            update_interval: std::time::Duration::from_millis(50),
            print_interval: std::time::Duration::from_secs(2),
            fps_buffer: Vec::new(),
            fps: 0.0,
        }
    }

    pub fn run_ui(&mut self, ui: &mut egui::Ui) {
        let now = std::time::Instant::now();

        let delta = now - self.last_frame;
        self.last_frame = now;

        let frame_time = delta.as_secs_f32();
        let fps = 1.0 / frame_time;
        self.fps_buffer.push(fps);

        if now - self.last_update > self.update_interval {
            self.last_update = now;
            self.fps = self.fps_buffer.iter().sum::<f32>() / self.fps_buffer.len() as f32;
            self.fps_buffer.clear();
        }

        if now - self.last_print > self.print_interval {
            self.last_print = now;
            log::info!("FPS: {:.2}", self.fps);
        }

        ui.horizontal(|ui| {
            ui.label(format!("FPS: {:.2}", self.fps));
            ui.separator();
            ui.label(format!("Frame Time: {:.2}ms", frame_time * 1000.0));
        });
    }
}
