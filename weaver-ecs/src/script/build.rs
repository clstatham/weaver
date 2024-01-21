use std::sync::Arc;

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use crate::{
    id::DynamicId,
    prelude::World,
    query::{DynamicQuery, DynamicQueryParams},
    system::{DynamicSystem, ScriptParam, ScriptParamType},
};

use super::{
    parser::{Query, Scope, Statement, System, TypedIdent},
    Script,
};

pub fn query_foreach(script_query: &Query, query: &DynamicQuery) -> anyhow::Result<()> {
    for entries in query.iter() {
        let mut params = FxHashMap::default();
        for (ident, entry) in script_query.components.iter().zip(entries.iter()) {
            params.insert(&ident.name, entry);
        }

        for statement in script_query.block.statements.iter() {
            match statement {
                super::parser::Statement::Call(call) => match call.name.as_str() {
                    "dbg" => {
                        for arg in &call.args {
                            let value = params
                                .get(arg)
                                .ok_or(anyhow::anyhow!("Argument `{}` not found", arg))?;
                            println!("{}: {:?}", arg, value);
                        }
                    }
                    _ => {
                        todo!("Implement other calls");
                    }
                },
                _ => {
                    todo!("Implement other statements");
                }
            }
        }
    }

    Ok(())
}

pub fn interpret_script(system: &System, params: &[ScriptParam]) -> anyhow::Result<()> {
    for statement in &system.block.statements {
        match statement {
            Statement::Query(script_query) => {
                let query = params
                    .iter()
                    .find_map(|param| match &**param {
                        ScriptParamType::Query(query) => {
                            if param.name == script_query.name {
                                Some(query)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                    .unwrap();
                query_foreach(script_query, query)?;
            }
            _ => {
                todo!("Implement other statements");
            }
        }
    }

    Ok(())
}

pub trait BuildOnWorld {
    type Output;

    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<Self::Output>;
}

impl BuildOnWorld for Script {
    type Output = Vec<DynamicSystem>;

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
                        script = script.query(&query.name, query.build(world.clone())?);
                    }

                    let system = system.clone();
                    let script = script.build(world.clone(), move |params| {
                        interpret_script(&system, params)
                    });
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

impl BuildOnWorld for Query {
    type Output = DynamicQueryParams;

    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<DynamicQueryParams> {
        let mut params = DynamicQueryParams::default();
        for component in &self.components {
            let id = component.build(world.clone())?;
            params = params.read(id);
        }
        Ok(params)
    }
}

impl BuildOnWorld for TypedIdent {
    type Output = DynamicId;

    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<DynamicId> {
        Ok(world.read().named_id(&self.ty))
    }
}
