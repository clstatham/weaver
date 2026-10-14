//! Stage registry: which stages exist, in what order they run, and which systems are in each.

use weaver_util::prelude::*;

use crate::{
    prelude::{IntoSystem, World},
    system::{IntoSystemConfig, SystemGraph},
};

define_label!(
    /// Names a stage, a group of systems that run together. Derive it with
    /// `#[derive(SystemStage)]` on a unit struct or fieldless enum.
    SystemStage,
    SYSTEM_STAGE_INTERNER
);
/// An interned [`SystemStage`] label, cheap to copy and compare.
pub type InternedSystemStage = Interned<dyn SystemStage>;

/// All of a world's stages and the systems in them.
///
/// Stages come in four kinds. Init stages run once at startup, update stages run every frame in
/// the order they were pushed (or inserted with
/// [`add_update_stage_before`](Systems::add_update_stage_before) /
/// [`add_update_stage_after`](Systems::add_update_stage_after)), shutdown stages run once on exit,
/// and manual stages only run when asked for with [`run_stage`](Systems::run_stage). Each stage has
/// one [`SystemGraph`].
#[derive(Default)]
pub struct Systems {
    init_stages: Vec<InternedSystemStage>,
    update_stages: Vec<InternedSystemStage>,
    shutdown_stages: Vec<InternedSystemStage>,
    manual_stages: Vec<InternedSystemStage>,
    systems: FxHashMap<InternedSystemStage, SystemGraph>,
}

impl Systems {
    /// Initializes the systems in every stage of every kind.
    pub fn initialize(&mut self, world: &mut World) {
        self.initialize_init_stages(world);
        self.initialize_update_stages(world);
        self.initialize_shutdown_stages(world);
        self.initialize_manual_stages(world);
    }

    /// Returns true if the stage has been pushed.
    pub fn has_stage(&self, stage: impl SystemStage) -> bool {
        self.systems.contains_key(&stage.intern())
    }

    /// Returns the stage's system graph.
    ///
    /// # Panics
    ///
    /// Panics if the stage was never pushed.
    pub fn get_stage(&self, stage: impl SystemStage) -> &SystemGraph {
        self.systems
            .get(&stage.intern())
            .expect("System stage not found")
    }

    /// Returns the stage's system graph mutably.
    ///
    /// # Panics
    ///
    /// Panics if the stage was never pushed.
    pub fn get_stage_mut(&mut self, stage: impl SystemStage) -> &mut SystemGraph {
        self.systems
            .get_mut(&stage.intern())
            .expect("System stage not found")
    }

    /// Adds an update stage after all existing ones.
    // FIXME: pushing a label twice runs it twice per frame and swaps in an empty graph, losing its
    // systems
    pub fn push_update_stage(&mut self, stage: impl SystemStage) {
        let stage = stage.intern();
        self.update_stages.push(stage);
        self.systems.insert(stage, SystemGraph::default());
    }

    /// Adds an init stage after all existing ones.
    pub fn push_init_stage(&mut self, stage: impl SystemStage) {
        let stage = stage.intern();
        self.init_stages.push(stage);
        self.systems.insert(stage, SystemGraph::default());
    }

    /// Adds a shutdown stage after all existing ones.
    pub fn push_shutdown_stage(&mut self, stage: impl SystemStage) {
        let stage = stage.intern();
        self.shutdown_stages.push(stage);
        self.systems.insert(stage, SystemGraph::default());
    }

    /// Adds a stage that only runs through [`run_stage`](Systems::run_stage).
    pub fn push_manual_stage(&mut self, stage: impl SystemStage) {
        let stage = stage.intern();
        self.manual_stages.push(stage);
        self.systems.insert(stage, SystemGraph::default());
    }

    /// Inserts an update stage directly before the update stage `before`.
    ///
    /// # Panics
    ///
    /// Panics if `before` isn't an update stage.
    pub fn add_update_stage_before(&mut self, stage: impl SystemStage, before: impl SystemStage) {
        let stage = stage.intern();
        let before = before.intern();

        let index = self
            .update_stages
            .iter()
            .position(|s| s == &before)
            .expect("Stage not found");

        self.update_stages.insert(index, stage);

        self.systems.insert(stage, SystemGraph::default());
    }

    /// Inserts an update stage directly after the update stage `after`.
    ///
    /// # Panics
    ///
    /// Panics if `after` isn't an update stage.
    pub fn add_update_stage_after(&mut self, stage: impl SystemStage, after: impl SystemStage) {
        let stage = stage.intern();
        let after = after.intern();

        let index = self
            .update_stages
            .iter()
            .position(|s| s == &after)
            .expect("Stage not found");

        self.update_stages.insert(index + 1, stage);

        self.systems.insert(stage, SystemGraph::default());
    }

    /// Makes `run_first` run before `run_second` within `stage`.
    ///
    /// # Panics
    ///
    /// Panics if the stage was never pushed.
    pub fn order_systems<BEFORE, AFTER, M1, M2>(
        &mut self,
        run_first: BEFORE,
        run_second: AFTER,
        stage: impl SystemStage,
    ) where
        BEFORE: IntoSystem<M1>,
        AFTER: IntoSystem<M2>,
        M1: 'static,
        M2: 'static,
    {
        self.get_stage_mut(stage)
            .add_edge::<M1, M2, BEFORE, AFTER>(run_first, run_second);
    }

    /// Adds a system (or a [`SystemConfig`](crate::system::SystemConfig)) to `stage`.
    ///
    /// # Panics
    ///
    /// Panics if the stage was never pushed.
    pub fn add_system<S, M>(&mut self, system: S, stage: impl SystemStage)
    where
        S: IntoSystemConfig<M>,
        M: 'static,
    {
        self.get_stage_mut(stage).add_system(system);
    }

    /// Removes a system from `stage`.
    ///
    /// # Panics
    ///
    /// Panics if the stage was never pushed.
    pub fn remove_system<S, M>(&mut self, system: S, stage: impl SystemStage)
    where
        S: IntoSystem<M>,
        M: 'static,
    {
        self.get_stage_mut(stage).remove_system(system);
    }

    /// Returns true if `stage` contains the system.
    ///
    /// # Panics
    ///
    /// Panics if the stage was never pushed.
    pub fn has_system<M: 'static>(
        &self,
        system: &impl IntoSystem<M>,
        stage: impl SystemStage,
    ) -> bool {
        self.get_stage(stage).has_system(system)
    }

    /// Initializes the systems of one stage.
    ///
    /// # Panics
    ///
    /// Panics if the stage was never pushed, or its ordering is invalid
    /// (see [`SystemGraph::initialize`]).
    pub fn initialize_stage(&mut self, world: &mut World, stage: impl SystemStage) {
        self.get_stage_mut(stage).initialize(world);
    }

    /// Runs one stage, whatever its kind.
    ///
    /// # Errors
    ///
    /// Returns the error from [`SystemGraph::run`].
    ///
    /// # Panics
    ///
    /// Panics if the stage was never pushed.
    pub fn run_stage(&mut self, world: &mut World, stage: impl SystemStage) -> Result<()> {
        self.get_stage_mut(stage).run(world)
    }

    /// Initializes the systems of every init stage.
    pub fn initialize_init_stages(&mut self, world: &mut World) {
        for stage in &self.init_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .initialize(world);
        }
    }

    /// Runs every init stage in order.
    ///
    /// # Errors
    ///
    /// Stops at and returns the first error from [`SystemGraph::run`].
    pub fn run_init(&mut self, world: &mut World) -> Result<()> {
        for stage in &self.init_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .run(world)?;
        }
        Ok(())
    }

    /// Initializes the systems of every shutdown stage.
    pub fn initialize_shutdown_stages(&mut self, world: &mut World) {
        for stage in &self.shutdown_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .initialize(world);
        }
    }

    /// Runs every shutdown stage in order.
    ///
    /// # Errors
    ///
    /// Stops at and returns the first error from [`SystemGraph::run`].
    pub fn run_shutdown(&mut self, world: &mut World) -> Result<()> {
        for stage in &self.shutdown_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .run(world)?;
        }
        Ok(())
    }

    /// Initializes the systems of every update stage.
    pub fn initialize_update_stages(&mut self, world: &mut World) {
        for stage in &self.update_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .initialize(world);
        }
    }

    /// Runs every update stage in order.
    ///
    /// # Errors
    ///
    /// Stops at and returns the first error from [`SystemGraph::run`].
    pub fn run_update(&mut self, world: &mut World) -> Result<()> {
        for stage in &self.update_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .run(world)?;
        }
        Ok(())
    }

    /// Initializes the systems of every manual stage.
    pub fn initialize_manual_stages(&mut self, world: &mut World) {
        for stage in &self.manual_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .initialize(world);
        }
    }
}
