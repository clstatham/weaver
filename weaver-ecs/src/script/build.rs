use std::sync::Arc;

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use crate::{
    id::DynamicId,
    prelude::World,
    query::{DynamicQuery, DynamicQueryParams, DynamicQueryRef},
    system::{DynamicSystem, ScriptParam},
};

use super::{
    parser::{Call, Query, Scope, Statement, System, TypedIdent},
    Script,
};

pub struct InterpreterContext<'a, 'b> {
    pub world: Arc<RwLock<World>>,
    pub params: FxHashMap<String, &'b ScriptParam<'a>>,
    pub scoped_idents: FxHashMap<String, DynamicQueryRef<'a>>,
}

impl<'a, 'b> InterpreterContext<'a, 'b>
where
    'b: 'a,
{
    pub fn interp_statement(&mut self, statement: &Statement) -> anyhow::Result<()> {
        match statement {
            Statement::Query(script_query) => {
                let query = self.params.get(&script_query.name).unwrap().unwrap_query();
                self.interp_query_foreach(script_query, query)?;
            }
            Statement::Call(call) => {
                self.interp_call(call)?;
            }
            _ => {
                todo!("Implement other statements");
            }
        }
        Ok(())
    }

    pub fn interp_call(&mut self, call: &Call) -> anyhow::Result<()> {
        match call.name.as_str() {
            "print" => {
                let mut params = Vec::new();
                for arg in &call.args {
                    let param = self.scoped_idents.get(arg).unwrap();
                    params.push(format!("{:?}", param));
                }
                println!("{}", params.join("\n"));
            }
            _ => {
                todo!("Implement other calls");
            }
        }
        Ok(())
    }

    pub fn interp_query_foreach(
        &mut self,
        script_query: &Query,
        query: &'a DynamicQuery,
    ) -> anyhow::Result<()> {
        for entries in query.iter() {
            let mut idents = Vec::new();
            for (ident, entry) in script_query.components.iter().zip(entries) {
                idents.push(ident.name.clone());
                self.scoped_idents.insert(ident.name.clone(), entry);
            }

            for statement in script_query.block.statements.iter() {
                self.interp_statement(statement)?;
            }

            for ident in idents {
                self.scoped_idents.remove(&ident);
            }
        }

        Ok(())
    }

    pub fn interp_script(&mut self, system: &System) -> anyhow::Result<()> {
        for statement in &system.block.statements {
            self.interp_statement(statement)?;
        }

        Ok(())
    }
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
                    let world_clone = world.clone();
                    let script = script.build(world.clone(), move |params| {
                        let mut ctx = InterpreterContext {
                            world: world_clone.clone(),
                            params: params
                                .iter()
                                .map(|param| (param.name.clone(), param))
                                .collect(),
                            scoped_idents: FxHashMap::default(),
                        };
                        ctx.interp_script(&system)
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
