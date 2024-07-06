use weaver_app::{plugin::Plugin, App, PreUpdate};
use weaver_ecs::{
    component::{Res, ResMut},
    prelude::Resource,
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

#[derive(Resource)]
pub struct FixedTimestep<Label: Send + Sync + 'static> {
    pub timestep: f32,
    pub total_time: f32,
    accumulator: f32,
    max_frame_time: f32,
    _label: std::marker::PhantomData<Label>,
}

impl<Label: Send + Sync + 'static> FixedTimestep<Label> {
    pub fn new(timestep: f32, max_frame_time: f32) -> Self {
        Self {
            timestep,
            total_time: 0.0,
            accumulator: 0.0,
            max_frame_time,
            _label: std::marker::PhantomData,
        }
    }

    pub fn update(&mut self, time: &Time) {
        self.accumulator += time.delta_time;
        if self.accumulator > self.max_frame_time {
            self.accumulator = self.max_frame_time;
        }
    }

    /// Run the given function with a fixed timestep.
    /// The function should take two arguments: the total time and the timestep.
    ///
    /// The function will be called multiple times if multiple timesteps have passed since the last time this FixedTimestep was updated.
    ///
    /// See https://www.gafferongames.com/post/fix_your_timestep/
    pub fn run_with_fixed_timestep<F>(&mut self, mut f: F)
    where
        F: FnMut(f32, f32),
    {
        while self.accumulator >= self.timestep {
            f(self.total_time, self.timestep);
            self.accumulator -= self.timestep;
            self.total_time += self.timestep;
        }
    }
}

pub struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.insert_resource(Time::new());
        app.add_system(update_time, PreUpdate);
        Ok(())
    }
}

fn update_time(mut time: ResMut<Time>) -> Result<()> {
    time.update();
    Ok(())
}

pub struct FixedUpdatePlugin<Label: Send + Sync + 'static> {
    pub timestep: f32,
    pub max_frame_time: f32,
    _label: std::marker::PhantomData<Label>,
}

impl<Label: Send + Sync + 'static> FixedUpdatePlugin<Label> {
    pub fn new(timestep: f32, max_frame_time: f32) -> Self {
        Self {
            timestep,
            max_frame_time,
            _label: std::marker::PhantomData,
        }
    }
}

impl<Label: Send + Sync + 'static> Plugin for FixedUpdatePlugin<Label> {
    fn build(&self, app: &mut App) -> Result<()> {
        app.insert_resource(FixedTimestep::<Label>::new(
            self.timestep,
            self.max_frame_time,
        ));
        app.add_system_after(update_fixed_timestep::<Label>, update_time, PreUpdate);
        Ok(())
    }
}

fn update_fixed_timestep<Label: Send + Sync + 'static>(
    time: Res<Time>,
    mut fixed_timestep: ResMut<FixedTimestep<Label>>,
) -> Result<()> {
    fixed_timestep.update(&time);
    Ok(())
}
