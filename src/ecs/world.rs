use std::{cell::RefCell, sync::atomic::AtomicU32};

use super::{
    component::Component,
    entity::Entity,
    system::{Query, ResolvedQuery, System},
};
use rustc_hash::FxHashMap;

/// A collection of [Entity]s, [Component]s, and [System]s.
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct World {
    components: FxHashMap<Entity, Vec<RefCell<Component>>>,
    systems: Vec<System>,
}

impl World {
    /// Creates a new, empty [World].
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new [Entity] and adds it to the [World].
    pub fn create_entity(&mut self) -> Entity {
        static NEXT_ID: AtomicU32 = AtomicU32::new(0);
        let entity = Entity::new(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        self.components.insert(entity, Vec::new());
        entity
    }

    /// Adds a [Component] to an [Entity].
    pub fn add_component(&mut self, entity: Entity, mut component: Component) {
        component.entity = entity;
        self.components
            .entry(entity)
            .or_default()
            .push(RefCell::new(component));
    }

    /// Adds a [System] to the [World].
    pub fn add_system(&mut self, system: System) {
        self.systems.push(system);
    }

    /// Queries the [World] for [Component]s matching the given [Query].
    pub fn query<'a, 'b: 'a>(&'b self, query: &'a Query) -> ResolvedQuery<'a> {
        match query {
            Query::Immutable(component_name) => {
                let mut results = Vec::new();
                for components in self.components.values() {
                    for component in components {
                        if component_name == component.borrow().name() {
                            results.push(component.borrow());
                        }
                    }
                }
                if results.is_empty() {
                    ResolvedQuery::NoMatch
                } else {
                    ResolvedQuery::Immutable(results)
                }
            }
            Query::Mutable(component_name) => {
                let mut results = Vec::new();
                for components in self.components.values() {
                    for component in components {
                        if component_name == component.borrow().name() {
                            results.push(component.borrow_mut());
                        }
                    }
                }
                if results.is_empty() {
                    ResolvedQuery::NoMatch
                } else {
                    ResolvedQuery::Mutable(results)
                }
            }
        }
    }

    /// Runs all of the [World]'s [System]s.
    pub fn update(&self) {
        for system in &self.systems {
            system.update(self);
        }
    }
}
