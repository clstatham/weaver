use std::sync::atomic::AtomicU32;

use super::{
    component::Component,
    entity::Entity,
    system::{Query, ResolvedQuery, System},
};
use rustc_hash::FxHashMap;

/// A collection of entities, components, and systems.
#[derive(Default)]
pub struct World {
    components: FxHashMap<Entity, Vec<Component>>,
    systems: Vec<System>,
}

impl World {
    /// Creates a new, empty world.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new entity and adds it to the world.
    pub fn create_entity(&mut self) -> Entity {
        static NEXT_ID: AtomicU32 = AtomicU32::new(0);
        let entity = Entity::new(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        self.components.insert(entity, Vec::new());
        entity
    }

    /// Adds a component to an Entity.
    pub fn add_component(&mut self, entity: Entity, mut component: Component) {
        component.entity = entity;
        self.components.entry(entity).or_default().push(component);
    }

    pub fn add_system(&mut self, system: System) {
        self.systems.push(system);
    }

    pub fn query<'a, 'b: 'a>(&'b self, query: &'a Query) -> ResolvedQuery<'a> {
        match query {
            Query::Immutable(component_name) => {
                let mut results = Vec::new();
                for components in self.components.values() {
                    for component in components {
                        if component_name == component.name() {
                            results.push(component);
                        }
                    }
                }
                if results.is_empty() {
                    ResolvedQuery::NoMatch
                } else {
                    ResolvedQuery::Immutable(results)
                }
            }
        }
    }

    pub fn update(&self) {
        for system in &self.systems {
            system.update(self);
        }
    }
}
