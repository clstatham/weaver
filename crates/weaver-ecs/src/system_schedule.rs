use weaver_util::prelude::*;

use crate::{
    prelude::{IntoSystem, World},
    system::{IntoSystemConfig, SystemGraph},
};

define_label!(SystemStage, SYSTEM_STAGE_INTERNER);
pub type InternedSystemStage = Interned<dyn SystemStage>;

#[derive(Default)]
pub struct Systems {
    init_stages: Vec<InternedSystemStage>,
    update_stages: Vec<InternedSystemStage>,
    shutdown_stages: Vec<InternedSystemStage>,
    manual_stages: Vec<InternedSystemStage>,
    systems: FxHashMap<InternedSystemStage, SystemGraph>,
}

impl Systems {
    pub fn initialize(&mut self, world: &mut World) {
        self.initialize_init_stages(world);
        self.initialize_update_stages(world);
        self.initialize_shutdown_stages(world);
        self.initialize_manual_stages(world);
    }

    pub fn has_stage(&self, stage: impl SystemStage) -> bool {
        self.systems.contains_key(&stage.intern())
    }

    pub fn get_stage(&self, stage: impl SystemStage) -> &SystemGraph {
        self.systems
            .get(&stage.intern())
            .expect("System stage not found")
    }

    pub fn get_stage_mut(&mut self, stage: impl SystemStage) -> &mut SystemGraph {
        self.systems
            .get_mut(&stage.intern())
            .expect("System stage not found")
    }

    pub fn push_update_stage(&mut self, stage: impl SystemStage) {
        let stage = stage.intern();
        self.update_stages.push(stage);
        self.systems.insert(stage, SystemGraph::default());
    }

    pub fn push_init_stage(&mut self, stage: impl SystemStage) {
        let stage = stage.intern();
        self.init_stages.push(stage);
        self.systems.insert(stage, SystemGraph::default());
    }

    pub fn push_shutdown_stage(&mut self, stage: impl SystemStage) {
        let stage = stage.intern();
        self.shutdown_stages.push(stage);
        self.systems.insert(stage, SystemGraph::default());
    }

    pub fn push_manual_stage(&mut self, stage: impl SystemStage) {
        let stage = stage.intern();
        self.manual_stages.push(stage);
        self.systems.insert(stage, SystemGraph::default());
    }

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

    pub fn add_system<S, M>(&mut self, system: S, stage: impl SystemStage)
    where
        S: IntoSystemConfig<M>,
        M: 'static,
    {
        self.get_stage_mut(stage).add_system(system);
    }

    pub fn has_system<M: 'static>(
        &self,
        system: &impl IntoSystem<M>,
        stage: impl SystemStage,
    ) -> bool {
        self.get_stage(stage).has_system(system)
    }

    pub fn initialize_stage(&mut self, world: &mut World, stage: impl SystemStage) {
        self.get_stage_mut(stage).initialize(world);
    }

    pub fn run_stage(&mut self, world: &mut World, stage: impl SystemStage) -> Result<()> {
        self.get_stage_mut(stage).run(world)
    }

    pub fn initialize_init_stages(&mut self, world: &mut World) {
        for stage in &self.init_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .initialize(world);
        }
    }

    pub fn run_init(&mut self, world: &mut World) -> Result<()> {
        for stage in &self.init_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .run(world)?;
        }
        Ok(())
    }

    pub fn initialize_shutdown_stages(&mut self, world: &mut World) {
        for stage in &self.shutdown_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .initialize(world);
        }
    }

    pub fn run_shutdown(&mut self, world: &mut World) -> Result<()> {
        for stage in &self.shutdown_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .run(world)?;
        }
        Ok(())
    }

    pub fn initialize_update_stages(&mut self, world: &mut World) {
        for stage in &self.update_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .initialize(world);
        }
    }

    pub fn run_update(&mut self, world: &mut World) -> Result<()> {
        for stage in &self.update_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .run(world)?;
        }
        Ok(())
    }

    pub fn initialize_manual_stages(&mut self, world: &mut World) {
        for stage in &self.manual_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .initialize(world);
        }
    }
}
