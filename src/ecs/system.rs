use std::cell::{Ref, RefMut};

use crate::ecs::world::World;

use super::component::Component;

#[derive(Debug)]
pub enum Query {
    Immutable(String),
    Mutable(String),
}

#[derive(Debug)]
pub enum ResolvedQuery<'a> {
    NoMatch,
    Immutable(Vec<Ref<'a, Component>>),
    Mutable(Vec<RefMut<'a, Component>>),
}

// impl<'a> ResolvedQuery<'a> {
//     pub fn unwrap(self) -> Vec<Ref<'a, Component>> {
//         match self {
//             ResolvedQuery::NoMatch => panic!("unwrap called on ResolvedQuery::None"),
//             ResolvedQuery::Immutable(components) => components,
//         }
//     }
// }

pub type StaticSystemLogic = fn(&mut [ResolvedQuery]);

pub enum SystemLogic {
    None,
    Static(StaticSystemLogic),
    // todo: dynamic system logic
}

pub struct System {
    name: String,
    logic: SystemLogic,
    pub(crate) queries: Vec<Query>,
}

impl System {
    pub fn new(name: String, logic: SystemLogic) -> Self {
        System {
            name,
            logic,
            queries: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn logic(&mut self) -> &mut SystemLogic {
        &mut self.logic
    }

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
