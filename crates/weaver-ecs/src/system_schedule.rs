use std::any::TypeId;

use weaver_util::{FxHashMap, Result};

use crate::{
    prelude::{IntoSystem, System, World},
    system::SystemGraph,
};

pub trait SystemStage: 'static {}

#[derive(Default)]
pub struct Systems {
    init_stages: Vec<TypeId>,
    update_stages: Vec<TypeId>,
    shutdown_stages: Vec<TypeId>,
    manual_stages: Vec<TypeId>,
    systems: FxHashMap<TypeId, SystemGraph>,
}

impl Systems {
    pub fn has_stage<T: SystemStage>(&self) -> bool {
        self.systems.contains_key(&TypeId::of::<T>())
    }

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

    pub fn add_update_stage_before<T: SystemStage, BEFORE: SystemStage>(&mut self) {
        let index = self
            .update_stages
            .iter()
            .position(|stage| *stage == TypeId::of::<BEFORE>())
            .expect("System stage not found");
        self.update_stages.insert(index, TypeId::of::<T>());
        self.systems
            .insert(TypeId::of::<T>(), SystemGraph::default());
    }

    pub fn add_update_stage_after<T: SystemStage, AFTER: SystemStage>(&mut self) {
        let index = self
            .update_stages
            .iter()
            .position(|stage| *stage == TypeId::of::<AFTER>())
            .expect("System stage not found");
        self.update_stages.insert(index + 1, TypeId::of::<T>());
        self.systems
            .insert(TypeId::of::<T>(), SystemGraph::default());
    }

    pub fn add_system<T, S, M>(&mut self, system: S, _stage: T)
    where
        T: SystemStage,
        S: IntoSystem<M>,
        S::System: System<Output = ()>,
        M: 'static,
    {
        self.systems
            .get_mut(&TypeId::of::<T>())
            .expect("System stage not found")
            .add_system(system);
    }

    pub fn add_system_before<T, S, BEFORE, M1, M2>(&mut self, system: S, before: BEFORE, _stage: T)
    where
        T: SystemStage,
        S: IntoSystem<M1>,
        BEFORE: IntoSystem<M2>,
        S::System: System<Output = ()>,
        BEFORE::System: System<Output = ()>,
        M1: 'static,
        M2: 'static,
    {
        self.systems
            .get_mut(&TypeId::of::<T>())
            .expect("System stage not found")
            .add_system_before(system, before);
    }

    pub fn add_system_after<T, S, AFTER, M1, M2>(&mut self, system: S, after: AFTER, _stage: T)
    where
        T: SystemStage,
        S: IntoSystem<M1>,
        AFTER: IntoSystem<M2>,
        S::System: System<Output = ()>,
        AFTER::System: System<Output = ()>,
        M1: 'static,
        M2: 'static,
    {
        self.systems
            .get_mut(&TypeId::of::<T>())
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
