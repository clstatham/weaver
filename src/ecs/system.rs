use std::cell::{Ref, RefMut};

use crate::ecs::world::World;

use super::component::Component;

/// A query for [Component]s, used by [System]s to find [Component]s to operate on.
#[derive(Debug)]
pub enum Query {
    Immutable(String),
    Mutable(String),
}

/// The result of a [Query], possibly containing references to [Component]s in the queried [World].
#[derive(Debug)]
pub enum ResolvedQuery<'a> {
    NoMatch,
    Immutable(Vec<Ref<'a, Component>>),
    Mutable(Vec<RefMut<'a, Component>>),
}

/// A static function that implements a [System]'s logic.
pub type StaticSystemLogic = fn(&mut [ResolvedQuery]);

/// How a [System] is implemented.
pub enum SystemLogic {
    None,
    Static(StaticSystemLogic),
    // todo: dynamic system logic
}

/// A System, which operates on components in a [World].
pub struct System {
    name: String,
    logic: SystemLogic,
    queries: Vec<Query>,
}

impl System {
    /// Creates a new [System] with the given name and [SystemLogic].
    pub fn new(name: String, logic: SystemLogic) -> Self {
        System {
            name,
            logic,
            queries: Vec::new(),
        }
    }

    /// Returns the name of the [System].
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a reference to the [System]'s logic.
    pub fn logic(&self) -> &SystemLogic {
        &self.logic
    }

    /// Adds a [Query] to the System. The [System] will operate on components matching the [Query].
    pub fn add_query(&mut self, query: Query) {
        self.queries.push(query);
    }

    /// Returns a slice of the [System]'s Queries.
    pub fn queries(&self) -> &[Query] {
        &self.queries
    }

    /// Runs the [System]'s logic a single time on the given [World].
    ///
    /// This will query the [World] for [Component]s matching the [System]'s [Query]ies, and then run the [System]'s logic on those [Component]s.
    pub fn update<'a, 'b: 'a>(&'a self, world: &'b World) {
        let mut components = Vec::new();
        for query in &self.queries {
            let result = world.query(query);
            match result {
                ResolvedQuery::NoMatch => {
                    log::warn!("query {:?} returned no results", query);
                    components.push(result);
                }
                _ => {
                    components.push(result);
                }
            }
        }
        match &self.logic {
            SystemLogic::None => {}
            SystemLogic::Static(logic) => logic(&mut components),
        }
    }
}
