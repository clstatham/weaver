use std::sync::{Arc, RwLock};

use rustc_hash::FxHashMap;

use crate::{Component, Entity, Read, System};

#[derive(Default)]
pub struct World {
    pub(crate) entities_components: FxHashMap<Entity, Vec<Arc<RwLock<dyn Component>>>>,
    pub(crate) systems: Vec<Box<dyn System>>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_entity(&mut self) -> Entity {
        static NEXT_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Entity::new(id, 0)
    }

    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) {
        let component = Arc::new(RwLock::new(component));
        self.entities_components
            .entry(entity)
            .or_default()
            .push(component);
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        if let Some(components) = self.entities_components.get_mut(&entity) {
            components.retain(|component| !component.read().unwrap().as_any().is::<T>());
        }
    }

    pub fn query<T: Component>(&self) -> Read<T> {
        Read::new(self)
    }

    pub fn add_system<S: System + 'static>(&mut self, system: S) {
        self.systems.push(Box::new(system));
    }

    pub fn update(&self) {
        for system in &self.systems {
            system.run(self);
        }
    }
}
