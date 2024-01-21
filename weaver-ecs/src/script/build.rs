use std::sync::Arc;

use parking_lot::RwLock;

use crate::{
    id::DynamicId,
    prelude::World,
    query::{DynamicQueryParams, DynamicQueryRef},
    system::{DynamicSystem, ScriptParams},
};

use super::{
    parser::{Query, Scope, TypedIdent},
    Script,
};

pub fn interpret_script(params: &[ScriptParams]) -> anyhow::Result<()> {
    for param in params {
        match param {
            ScriptParams::Query(query) => {
                query.iter().for_each(|components| {
                    for component in components {
                        match component {
                            DynamicQueryRef::Ref(component) => {
                                println!("ref {:?}", component);
                            }
                            DynamicQueryRef::Mut(component) => {
                                println!("mut {:?}", component);
                            }
                        }
                    }
                });
            }
            _ => {
                todo!("Implement other params");
            }
        }
    }

    Ok(())
}

pub trait BuildOnWorld<T> {
    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<T>;
}

impl BuildOnWorld<Vec<DynamicSystem>> for Script {
    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<Vec<DynamicSystem>> {
        if !self.is_parsed() {
            return Err(anyhow::anyhow!("Script is not parsed"));
        }
        let mut systems = Vec::new();
        for scope in &self.scopes {
            match scope {
                Scope::System(ref system) => {
                    let mut script = DynamicSystem::script_builder(&system.name);

                    for query in &system.queries {
                        script = script.query(query.build(world.clone())?);
                    }

                    let script = script.build(world.clone(), interpret_script);
                    systems.push(script);
                }
                _ => {
                    todo!("Implement other scopes");
                }
            }
        }

        Ok(systems)
    }
}

impl BuildOnWorld<DynamicQueryParams> for Query {
    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<DynamicQueryParams> {
        let mut params = DynamicQueryParams::default();
        for component in &self.components {
            let id = component.build(world.clone())?;
            params = params.read(id);
        }
        Ok(params)
    }
}

impl BuildOnWorld<DynamicId> for TypedIdent {
    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<DynamicId> {
        Ok(world.read().named_id(&self.ty))
    }
}
