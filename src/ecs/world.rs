use std::sync::Arc;

use rustc_hash::FxHashMap;

use super::{
    component::Component,
    entity::Entity,
    query::{ReadResult, WriteResult},
    system::System,
};

pub struct Components {
    data: FxHashMap<Entity, Vec<Box<dyn Component>>>,
}

impl Components {
    pub fn new() -> Self {
        Self {
            data: FxHashMap::default(),
        }
    }

    pub fn insert<T: Component>(&mut self, entity: Entity, component: T) {
        let components = self.data.entry(entity).or_default();
        components.push(Box::new(component));
    }

    pub fn remove<T: Component>(&mut self, entity: Entity) {
        if let Some(components) = self.data.get_mut(&entity) {
            components.retain(|component| !component.as_any().is::<T>());
        }
    }

    pub(crate) fn read<'a, 'b: 'a, T: Component>(&'b self) -> ReadResult<'a, T> {
        let mut result = Vec::new();

        for components in self.data.values() {
            for lock in components.iter() {
                if let Some(component) = lock.as_any().downcast_ref::<T>() {
                    result.push(component);
                }
            }
        }
        ReadResult { components: result }
    }

    pub(crate) fn write<'a, 'b: 'a, T: Component>(&'b mut self) -> WriteResult<'a, T> {
        let mut result = Vec::new();

        for components in self.data.values_mut() {
            for lock in components.iter_mut() {
                if let Some(component) = lock.as_any_mut().downcast_mut::<T>() {
                    result.push(component);
                }
            }
        }
        WriteResult { components: result }
    }
}

impl Default for Components {
    fn default() -> Self {
        Self::new()
    }
}

pub struct World {
    components: Components,
    systems: Vec<Arc<dyn System>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            components: Components::new(),
            systems: Vec::new(),
        }
    }

    pub fn components(&self) -> &Components {
        &self.components
    }

    pub fn components_mut(&mut self) -> &mut Components {
        &mut self.components
    }

    pub fn register_system<T: System>(&mut self, system: T) {
        self.systems.push(Arc::new(system));
    }

    pub fn unregister_system(&mut self, system: &Arc<dyn System>) {
        self.systems.retain(|s| !Arc::ptr_eq(s, system));
    }

    pub fn read<T: Component>(&self) -> ReadResult<'_, T> {
        self.components.read()
    }

    pub fn write<T: Component>(&mut self) -> WriteResult<'_, T> {
        self.components.write()
    }

    pub fn spawn<T: Component>(&mut self, component: T) -> Entity {
        static NEXT_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let entity = Entity::new(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        self.components.insert(entity, component);
        entity
    }

    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) {
        self.components.insert(entity, component);
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        self.components.remove::<T>(entity);
    }

    pub fn update(&mut self, delta: std::time::Duration) {
        for system in self.systems.clone().iter() {
            system.run(self, delta);
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
