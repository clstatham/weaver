use std::{
    ops::{Add, Div, Mul, Sub},
    sync::Arc,
};

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use crate::{
    id::DynamicId,
    prelude::World,
    query::{DynamicQuery, DynamicQueryParams, DynamicQueryRef},
    system::{DynamicSystem, ScriptParam},
};

use super::{
    parser::{Call, Component, Expr, Query, Scope, Statement, System, TypedIdent},
    Script,
};

macro_rules! match_two_components_helper {
    ($cx:ident, $ty:ty, $value_ty:ident; $lhs:ident = $lhs_binding:ident; $rhs:ident = $rhs_binding:ident => $block:block else $els:block) => {
        match $lhs {
            Value::Component { ty, name: lhs_name } if ty.as_str() == stringify!($ty) => match $rhs
            {
                Value::Component { ty, name: rhs_name } if ty.as_str() == stringify!($ty) => {
                    let $lhs_binding = $cx.current_scope().get_param(&lhs_name).unwrap();
                    let $lhs_binding = $lhs_binding.data().get_as::<$ty>().unwrap();
                    let $rhs_binding = $cx.current_scope().get_param(&rhs_name).unwrap();
                    let $rhs_binding = $rhs_binding.data().get_as::<$ty>().unwrap();
                    Value::$value_ty($block)
                }
                _ => $els,
            },
            _ => $els,
        }
    };
}

macro_rules! match_two_components {
    ($cx:ident, $lhs:ident = $lhs_binding:ident; $rhs:ident = $rhs_binding:ident => $block:block) => {
        match_two_components_helper!($cx, f32, Float; $lhs = $lhs_binding; $rhs = $rhs_binding => $block else {
            match_two_components_helper!($cx, i64, Int; $lhs = $lhs_binding; $rhs = $rhs_binding => $block else {
                todo!("Implement other types")
            })
        })
    };
}

macro_rules! match_two_components_mut_helper {
    ($cx:ident, $ty:ty, $value_ty:ident; $lhs:ident = $lhs_binding:ident; $rhs:ident = $rhs_binding:ident => $block:block else $els:block) => {
        match $lhs {
            Value::Component { ty, name: lhs_name } if ty.as_str() == stringify!($ty) => match $rhs
            {
                Value::Component { ty, name: rhs_name } if ty.as_str() == stringify!($ty) => {
                    let scope = $cx.current_scope_mut();
                    let $rhs_binding = scope.get_param(&rhs_name).unwrap();
                    let $rhs_binding = *$rhs_binding.data().get_as::<$ty>().unwrap();
                    let $lhs_binding = scope.get_param_mut(&lhs_name).unwrap();
                    let $lhs_binding = $lhs_binding
                        .data_mut()
                        .unwrap()
                        .get_as_mut::<$ty>()
                        .unwrap();

                    $block;
                    Value::Void
                }
                _ => $els,
            },
            _ => $els,
        }
    };
}

macro_rules! match_two_components_mut {
    ($cx:ident, $lhs:ident = $lhs_binding:ident; $rhs:ident = $rhs_binding:ident => $block:block) => {
        match_two_components_mut_helper!($cx, f32, Float; $lhs = $lhs_binding; $rhs = $rhs_binding => $block else {
            match_two_components_mut_helper!($cx, i64, Int; $lhs = $lhs_binding; $rhs = $rhs_binding => $block else {
                todo!("Implement other types")
            })
        })
    };
}

#[derive(Debug, Clone)]
pub enum Value {
    Void,
    Float(f32),
    Int(i64),
    Bool(bool),
    Component {
        name: String,
        ty: String,
    },
    Field {
        name: String,
        ty: String,
        parent: Box<Value>,
    },
}

impl Value {
    pub fn type_name(&self) -> &str {
        match self {
            Value::Void => "()",
            Value::Float(_) => "f64",
            Value::Int(_) => "i64",
            Value::Bool(_) => "bool",
            Value::Component { ty, .. } => ty,
            Value::Field { ty, .. } => ty,
        }
    }

    pub fn add(&self, rhs: &Self, cx: &mut InterpreterContext<'_>) -> anyhow::Result<Self> {
        match (self, rhs) {
            (Value::Component { .. }, Value::Component { .. }) => {
                Ok(match_two_components!(cx, self = lhs; rhs = rhs => {
                    lhs.add(rhs)
                }))
            }
            _ => {
                todo!("Implement {:?} + {:?}", self, rhs);
            }
        }
    }

    pub fn sub(&self, rhs: &Self, cx: &mut InterpreterContext<'_>) -> anyhow::Result<Self> {
        match (self, rhs) {
            (Value::Component { .. }, Value::Component { .. }) => {
                Ok(match_two_components!(cx, self = lhs; rhs = rhs => {
                    lhs.sub(rhs)
                }))
            }
            _ => {
                todo!("Implement {:?} - {:?}", self, rhs);
            }
        }
    }

    pub fn add_assign(&self, rhs: &Self, cx: &mut InterpreterContext<'_>) -> anyhow::Result<Self> {
        match (self, rhs) {
            (Value::Component { .. }, Value::Component { .. }) => {
                Ok(match_two_components_mut!(cx, self = lhs; rhs = rhs => {
                    *lhs += rhs;
                }))
            }
            _ => {
                todo!("Implement {:?} += {:?}", self, rhs);
            }
        }
    }

    pub fn sub_assign(&self, rhs: &Self, cx: &mut InterpreterContext<'_>) -> anyhow::Result<Self> {
        match (self, rhs) {
            (Value::Component { .. }, Value::Component { .. }) => {
                Ok(match_two_components_mut!(cx, self = lhs; rhs = rhs => {
                    *lhs -= rhs;
                }))
            }
            _ => {
                todo!("Implement {:?} -= {:?}", self, rhs);
            }
        }
    }
}

pub struct InterpreterScope<'a> {
    pub values: FxHashMap<String, Value>,
    pub params: FxHashMap<String, DynamicQueryRef<'a>>,
}

impl<'a> InterpreterScope<'a> {
    pub fn new() -> Self {
        Self {
            values: FxHashMap::default(),
            params: FxHashMap::default(),
        }
    }

    pub fn get_param<'get>(&'get self, name: &str) -> Option<&'get DynamicQueryRef<'a>> {
        self.params.get(name)
    }

    pub fn get_param_mut<'get>(
        &'get mut self,
        name: &str,
    ) -> Option<&'get mut DynamicQueryRef<'a>> {
        self.params.get_mut(name)
    }
}

pub struct InterpreterContext<'a> {
    pub world: Arc<RwLock<World>>,
    pub params: FxHashMap<String, &'a ScriptParam<'a>>,
    pub scopes: Vec<InterpreterScope<'a>>,
}

impl<'a> InterpreterContext<'a> {
    fn push_scope(&mut self) {
        if let Some(scope) = self.scopes.last_mut() {
            let new_scope = InterpreterScope {
                values: scope.values.clone(),
                params: scope.params.drain().collect(),
            };
            self.scopes.push(new_scope);
        } else {
            self.scopes.push(InterpreterScope {
                values: FxHashMap::default(),
                params: FxHashMap::default(),
            });
        }
    }

    fn pop_scope(&mut self) {
        let old_scope = self.scopes.pop();
        if let Some(mut old_scope) = old_scope {
            if let Some(current_scope) = self.scopes.last_mut() {
                current_scope.values.extend(old_scope.values.drain());
                current_scope.params.extend(old_scope.params.drain());
            }
        }
    }

    fn current_scope<'scope>(&'scope self) -> &'scope InterpreterScope<'a> {
        self.scopes.last().unwrap()
    }

    fn current_scope_mut(&mut self) -> &mut InterpreterScope<'a> {
        self.scopes.last_mut().unwrap()
    }

    pub fn interp_statement(&mut self, statement: &Statement) -> anyhow::Result<Value> {
        match statement {
            Statement::Query(script_query) => {
                let query = self.params.get(&script_query.name).unwrap().unwrap_query();
                self.interp_query(script_query, query)?;
                Ok(Value::Void)
            }
            Statement::Expr(expr) => self.interp_expr(expr),
            _ => {
                todo!("Implement other statements");
            }
        }
    }

    pub fn interp_block(&mut self, statements: &[Statement]) -> anyhow::Result<Value> {
        self.push_scope();
        for statement in statements {
            self.interp_statement(statement)?;
        }
        self.pop_scope();
        Ok(Value::Void)
    }

    pub fn interp_expr(&mut self, expr: &Expr) -> anyhow::Result<Value> {
        let value = match expr {
            Expr::Call(call) => self.interp_call(call),
            Expr::FloatLiteral(float) => Ok(Value::Float(*float as f32)),
            Expr::IntLiteral(int) => Ok(Value::Int(*int)),
            Expr::StringLiteral(_string) => todo!("Implement string literals"),
            Expr::Block(block) => self.interp_block(&block.statements),
            Expr::Ident(ident) => {
                let scope = self.current_scope();
                let param = scope.get_param(ident).unwrap();
                let ty = param.type_name();
                Ok(Value::Component {
                    name: ident.clone(),
                    ty: ty.to_string(),
                })
            }
            Expr::Member { lhs, rhs } => {
                let lhs = self.interp_expr(lhs)?;
                let lhs_name = match lhs {
                    Value::Component { name, .. } => name,
                    _ => {
                        todo!("Implement other member access");
                    }
                };
                let scope = self.current_scope_mut();
                let lhs = scope.get_param(&lhs_name).unwrap();
                let rhs_name = match &**rhs {
                    Expr::Ident(ident) => ident,
                    _ => {
                        todo!("Implement other member access");
                    }
                };

                let lhs_ty = lhs.type_name();

                let value = Value::Field {
                    name: rhs_name.clone(),
                    ty: lhs_ty.to_string(),
                    parent: Box::new(Value::Component {
                        name: lhs_name.clone(),
                        ty: lhs_ty.to_string(),
                    }),
                };

                Ok(value)
            }
            Expr::Infix { op, lhs, rhs } => {
                let lhs = self.interp_expr(lhs)?;
                let rhs = self.interp_expr(rhs)?;
                match op.as_str() {
                    "+" => lhs.add(&rhs, self),
                    "-" => lhs.sub(&rhs, self),
                    "+=" => lhs.add_assign(&rhs, self),
                    "-=" => lhs.sub_assign(&rhs, self),
                    _ => {
                        todo!("Implement other infix operators");
                    }
                }
            }
            expr => {
                todo!("Implement other expressions: {:?}", expr);
            }
        }?;
        Ok(value)
    }

    pub fn interp_call(&mut self, call: &Call) -> anyhow::Result<Value> {
        match call.name.as_str() {
            "print" => {
                let mut params = Vec::new();
                for arg in &call.args {
                    let param = self.interp_expr(arg)?;
                    let param = match param {
                        Value::Component { name, .. } => {
                            let scope = self.current_scope();
                            let param = scope.get_param(&name).unwrap();
                            format!("{}: {} = {:?}", name, param.type_name(), param)
                        }
                        _ => format!("{:?}", param),
                    };
                    params.push(param);
                }
                println!("{}", params.join("\n"));
                Ok(Value::Void)
            }
            _ => {
                todo!("Implement other calls");
            }
        }
    }

    pub fn interp_query(
        &mut self,
        script_query: &Query,
        query: &'a DynamicQuery,
    ) -> anyhow::Result<()> {
        for entries in query.iter() {
            let mut idents = Vec::new();
            for (ident, entry) in script_query.components.iter().zip(entries) {
                idents.push(ident.name.clone());
                self.current_scope_mut().values.insert(
                    ident.name.clone(),
                    Value::Component {
                        name: ident.name.clone(),
                        ty: ident.ty.clone(),
                    },
                );

                self.current_scope_mut()
                    .params
                    .insert(ident.name.clone(), entry);
            }

            self.interp_block(&script_query.block.statements)?;
        }

        Ok(())
    }

    pub fn interp_system(&mut self, system: &System) -> anyhow::Result<()> {
        self.interp_block(&system.block.statements)?;

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
                    let script = system.build(world.clone())?;
                    systems.push(script);
                }
                Scope::Component(component) => {
                    component.build(world.clone())?;
                }
                _ => {
                    todo!("Implement other scopes");
                }
            }
        }

        Ok(systems)
    }
}

impl BuildOnWorld for Component {
    type Output = DynamicId;

    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<DynamicId> {
        let mut world = world.write();
        let mut builder = world.components.create_component(&self.name);
        for field in &self.fields {
            builder = builder.dynamic_field(&field.name, &field.ty);
        }

        todo!("Implement component builder")
    }
}

impl BuildOnWorld for System {
    type Output = DynamicSystem;

    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<DynamicSystem> {
        let mut script = DynamicSystem::script_builder(&self.name);

        for query in &self.queries {
            script = script.query(&query.name, query.build(world.clone())?);
        }

        let system = self.clone();
        let world_clone = world.clone();
        let script = script.build(world.clone(), move |params| {
            let mut ctx = InterpreterContext {
                world: world_clone.clone(),
                params: params
                    .iter()
                    .map(|param| (param.name.clone(), param))
                    .collect(),
                scopes: Vec::new(),
            };
            ctx.interp_system(&system)
        });

        Ok(script)
    }
}

impl BuildOnWorld for Query {
    type Output = DynamicQueryParams;

    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<DynamicQueryParams> {
        let mut params = DynamicQueryParams::default();
        for component in &self.components {
            let id = component.build(world.clone())?;
            if component.mutability {
                params = params.write(id);
            } else {
                params = params.read(id);
            }
        }
        for with in &self.with {
            let id = with.build(world.clone())?;
            params = params.with(id);
        }
        for without in &self.without {
            let id = without.build(world.clone())?;
            params = params.without(id);
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

impl BuildOnWorld for String {
    type Output = DynamicId;

    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<DynamicId> {
        Ok(world.read().named_id(self))
    }
}
