//! Frame timing and fixed-timestep clocks.
//!
//! [`Time`] is the wall-clock frame timer, updated once per frame by [`TimePlugin`].
//! [`FixedTimestep`] turns that variable frame time into whole fixed steps for simulation code,
//! and [`FixedUpdatePlugin`] keeps one up to date.

use weaver_app::{App, AppStage, plugin::Plugin};
use weaver_ecs::{
    component::{Res, ResMut},
    system::IntoSystemConfig,
};
use weaver_util::prelude::*;

/// Wall-clock time for the current frame.
///
/// Stored as a resource by [`TimePlugin`] and refreshed in [`AppStage::PreUpdate`].
pub struct Time {
    /// Seconds between the last two [`Time::update`] calls.
    pub delta_time: f32,
    /// Seconds accumulated over all frames so far.
    pub total_time: f32,
    /// Number of [`Time::update`] calls so far.
    pub frame_count: u32,
    last_update: std::time::Instant,
}

impl Time {
    #[allow(clippy::new_without_default)]
    /// Creates a clock whose first frame is measured from now.
    pub fn new() -> Self {
        Self {
            delta_time: 0.0,
            total_time: 0.0,
            frame_count: 0,
            last_update: std::time::Instant::now(),
        }
    }

    /// Advances the clock to the current instant and bumps the frame count.
    pub fn update(&mut self) {
        let now = std::time::Instant::now();
        self.delta_time = now.duration_since(self.last_update).as_secs_f32();
        self.total_time += self.delta_time;
        self.frame_count += 1;
        self.last_update = now;
    }
}

/// Accumulator for running simulation at a fixed rate, independent of the frame rate.
///
/// `Label` is a marker type that lets several independent fixed clocks live in the world as
/// separate resources (one `FixedTimestep<Physics>`, one `FixedTimestep<Ai>`, ...). Feed it with
/// [`FixedTimestep::update`] once per frame, then drain whole steps with
/// [`FixedTimestep::run_with_fixed_timestep`].
pub struct FixedTimestep<Label: Send + Sync + 'static> {
    /// Length of one step, in seconds.
    pub timestep: f32,
    /// Simulated seconds, advanced a whole step at a time by `run_with_fixed_timestep`.
    pub total_time: f32,
    accumulator: f32,
    max_frame_time: f32,
    _label: std::marker::PhantomData<Label>,
}

impl<Label: Send + Sync + 'static> FixedTimestep<Label> {
    /// Creates a fixed clock with steps of `timestep` seconds.
    ///
    /// `max_frame_time` caps the accumulator, so one long frame can't schedule an unbounded number
    /// of catch-up steps. It should be at least `timestep`, or the clock never becomes ready.
    pub fn new(timestep: f32, max_frame_time: f32) -> Self {
        Self {
            timestep,
            total_time: 0.0,
            accumulator: 0.0,
            max_frame_time,
            _label: std::marker::PhantomData,
        }
    }

    /// Adds this frame's [`Time::delta_time`] to the accumulator, capped at `max_frame_time`.
    pub fn update(&mut self, time: &Time) {
        self.accumulator += time.delta_time;
        if self.accumulator > self.max_frame_time {
            self.accumulator = self.max_frame_time;
        }
    }

    /// Returns `true` if at least one whole step is waiting in the accumulator.
    pub fn ready(&self) -> bool {
        self.accumulator >= self.timestep
    }

    /// Drops any accumulated time, e.g. after a pause or a scene load.
    pub fn clear_accumulator(&mut self) {
        self.accumulator = 0.0;
    }

    /// Run the given function with a fixed timestep.
    /// The function should take two arguments: the total time and the timestep.
    ///
    /// The function will be called multiple times if multiple timesteps have passed since the last time this FixedTimestep was updated.
    ///
    /// See <https://www.gafferongames.com/post/fix_your_timestep/>
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

/// Inserts the [`Time`] resource and updates it every frame in [`AppStage::PreUpdate`].
///
/// The clock starts when the plugin is built, so the first frame's delta includes everything since.
pub struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.insert_resource(Time::new());
        app.add_system(update_time, AppStage::PreUpdate);
        Ok(())
    }
}

fn update_time(mut time: ResMut<Time>) {
    time.update();
}

/// Inserts a [`FixedTimestep<Label>`] resource and feeds it every frame.
///
/// This only accumulates time. A system of your own has to call
/// [`FixedTimestep::run_with_fixed_timestep`] to actually run the steps. Needs [`TimePlugin`].
pub struct FixedUpdatePlugin<Label: 'static> {
    /// Length of one step, in seconds.
    pub timestep: f32,
    /// Cap on accumulated time per frame, in seconds.
    pub max_frame_time: f32,
    _label: std::marker::PhantomData<Label>,
}

impl<Label: 'static> FixedUpdatePlugin<Label> {
    /// Creates the plugin; see [`FixedTimestep::new`] for the arguments.
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
        app.add_system(
            update_fixed_timestep::<Label>.after(update_time),
            AppStage::PreUpdate,
        );
        Ok(())
    }
}

fn update_fixed_timestep<Label: Send + Sync + 'static>(
    time: Res<Time>,
    mut fixed_timestep: ResMut<FixedTimestep<Label>>,
) {
    fixed_timestep.update(&time);
}
