use std::any::TypeId;

use weaver_util::{FxHashMap, Result};

use crate::{
    prelude::{IntoSystem, World},
    system::SystemGraph,
};

pub trait SystemStage: 'static {}

#[derive(Default)]
pub struct SystemSchedule {
    init_stages: Vec<TypeId>,
    update_stages: Vec<TypeId>,
    shutdown_stages: Vec<TypeId>,
    manual_stages: Vec<TypeId>,
    systems: FxHashMap<TypeId, SystemGraph>,
}

impl SystemSchedule {
    pub fn push_update_stage<T: SystemStage>(&mut self) {
        self.update_stages.push(TypeId::of::<T>());
        self.systems
            .insert(TypeId::of::<T>(), SystemGraph::default());
    }

    pub fn push_init_stage<T: SystemStage>(&mut self) {
        self.init_stages.push(TypeId::of::<T>());
        self.systems
            .insert(TypeId::of::<T>(), SystemGraph::default());
    }

    pub fn push_shutdown_stage<T: SystemStage>(&mut self) {
        self.shutdown_stages.push(TypeId::of::<T>());
        self.systems
            .insert(TypeId::of::<T>(), SystemGraph::default());
    }

    pub fn push_manual_stage<T: SystemStage>(&mut self) {
        self.manual_stages.push(TypeId::of::<T>());
        self.systems
            .insert(TypeId::of::<T>(), SystemGraph::default());
    }

    pub fn add_stage_before<T: SystemStage, U: SystemStage>(&mut self) {
        let index = self
            .update_stages
            .iter()
            .position(|stage| *stage == TypeId::of::<U>())
            .expect("System stage not found");
        self.update_stages.insert(index, TypeId::of::<T>());
        self.systems
            .insert(TypeId::of::<T>(), SystemGraph::default());
    }

    pub fn add_stage_after<T: SystemStage, U: SystemStage>(&mut self) {
        let index = self
            .update_stages
            .iter()
            .position(|stage| *stage == TypeId::of::<U>())
            .expect("System stage not found");
        self.update_stages.insert(index + 1, TypeId::of::<T>());
        self.systems
            .insert(TypeId::of::<T>(), SystemGraph::default());
    }

    pub fn add_system<S: SystemStage, M: 'static>(
        &mut self,
        system: impl IntoSystem<M>,
        _stage: S,
    ) {
        self.systems
            .get_mut(&TypeId::of::<S>())
            .expect("System stage not found")
            .add_system(system);
    }

    pub fn add_system_before<S: SystemStage, M1: 'static, M2: 'static>(
        &mut self,
        system: impl IntoSystem<M1>,
        before: impl IntoSystem<M2>,
        _stage: S,
    ) {
        self.systems
            .get_mut(&TypeId::of::<S>())
            .expect("System stage not found")
            .add_system_before(system, before);
    }

    pub fn add_system_after<S: SystemStage, M1: 'static, M2: 'static>(
        &mut self,
        system: impl IntoSystem<M1>,
        after: impl IntoSystem<M2>,
        _stage: S,
    ) {
        self.systems
            .get_mut(&TypeId::of::<S>())
            .expect("System stage not found")
            .add_system_after(system, after);
    }

    pub fn has_system<S: SystemStage, M: 'static>(
        &self,
        system: &impl IntoSystem<M>,
        _stage: &S,
    ) -> bool {
        self.systems
            .get(&TypeId::of::<S>())
            .expect("System stage not found")
            .has_system(system)
    }

    pub fn run_stage<S: SystemStage>(&mut self, world: &mut World) -> Result<()> {
        self.systems
            .get_mut(&TypeId::of::<S>())
            .expect("System stage not found")
            .run(world)
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

    pub fn run_shutdown(&mut self, world: &mut World) -> Result<()> {
        for stage in &self.shutdown_stages {
            self.systems
                .get_mut(stage)
                .expect("System stage not found")
                .run(world)?;
        }
        Ok(())
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
}
