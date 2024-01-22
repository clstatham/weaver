use std::ops::{Add, Div, Mul, Rem, Sub};
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
    parser::{Call, Expr, Query, Scope, Statement, System, TypedIdent},
    Script,
};

#[derive(Debug, Clone)]
pub enum Value {
    Void,
    Bool(bool),
    U32(u32),
    U64(u64),
    Usize(usize),
    I32(i32),
    I64(i64),
    F32(f32),
    String(String),
    Component { name: String, ty: String },
}

// warning: arcane macro magic ahead that makes rust-analyzer cry

macro_rules! match_rhs {
    ($lhs:ident: $lhs_value:ident, $lhs_ty:ty ;  $rhs:ident; $expr:ident; $self:ident; $op:ident) => {
        match $rhs {
            Value::U32(rhs) => $expr!($lhs: $lhs_value, $lhs_ty; rhs: U32, u32; $self; $op),
            Value::U64(rhs) => $expr!($lhs: $lhs_value, $lhs_ty; rhs: U64, u64; $self; $op),
            Value::Usize(rhs) => $expr!($lhs: $lhs_value, $lhs_ty; rhs: Usize, usize; $self; $op),
            Value::I32(rhs) => $expr!($lhs: $lhs_value, $lhs_ty; rhs: I32, i32; $self; $op),
            Value::I64(rhs) => $expr!($lhs: $lhs_value, $lhs_ty; rhs: I64, i64; $self; $op),
            Value::F32(rhs) => $expr!($lhs: $lhs_value, $lhs_ty; rhs: F32, f32; $self; $op),
            Value::Component { .. } => match_rhs!($lhs: $lhs_value, $lhs_ty; ref $rhs; $expr; $self; $op),
            _ => {
                todo!("Implement other values for rhs: {:?}", $rhs);
            }
        }
    };

    (ref $lhs:ident: $lhs_value:ident, $lhs_ty:ty ; $rhs:ident; $expr:ident; $self:ident ; $op:ident) => {
        match $rhs {
            Value::U32(rhs) => $expr!(ref $lhs: $lhs_value, $lhs_ty; rhs: U32, u32; $self; $op),
            Value::U64(rhs) => $expr!(ref $lhs: $lhs_value, $lhs_ty; rhs: U64, u64; $self; $op),
            Value::Usize(rhs) => $expr!(ref $lhs: $lhs_value, $lhs_ty; rhs: Usize, usize; $self; $op),
            Value::I32(rhs) => $expr!(ref $lhs: $lhs_value, $lhs_ty; rhs: I32, i32; $self; $op),
            Value::I64(rhs) => $expr!(ref $lhs: $lhs_value, $lhs_ty; rhs: I64, i64; $self; $op),
            Value::F32(rhs) => $expr!(ref $lhs: $lhs_value, $lhs_ty; rhs: F32, f32; $self; $op),
            Value::Component { .. } => match_rhs!(ref $lhs: $lhs_value, $lhs_ty; ref $rhs; $expr; $self; $op),
            _ => {
                todo!("Implement other values for rhs: {:?}", $rhs);
            }
        }
    };

    (ref mut $lhs:ident: $lhs_value:ident, $lhs_ty:ty ; $rhs:ident; $expr:ident; $self:ident; $($op:ident)?) => {
        match $rhs {
            Value::U32(rhs) => $expr!(ref mut $lhs: $lhs_value, $lhs_ty; rhs: U32, u32; $self; $($op)?),
            Value::U64(rhs) => $expr!(ref mut $lhs: $lhs_value, $lhs_ty; rhs: U64, u64; $self; $($op)?),
            Value::Usize(rhs) => $expr!(ref mut $lhs: $lhs_value, $lhs_ty; rhs: Usize, usize; $self; $($op)?),
            Value::I32(rhs) => $expr!(ref mut $lhs: $lhs_value, $lhs_ty; rhs: I32, i32; $self; $($op)?),
            Value::I64(rhs) => $expr!(ref mut $lhs: $lhs_value, $lhs_ty; rhs: I64, i64; $self; $($op)?),
            Value::F32(rhs) => $expr!(ref mut $lhs: $lhs_value, $lhs_ty; rhs: F32, f32; $self; $($op)?),
            Value::Component { .. } => match_rhs!(ref mut $lhs: $lhs_value, $lhs_ty; ref $rhs; $expr; $self; $($op)?),
            _ => {
                todo!("Implement other values for rhs: {:?}", $rhs);
            }
        }
    };

    ($lhs:ident: $lhs_value:ident, $lhs_ty:ty ; ref $rhs:ident; $expr:ident; $self:ident ; $op:ident) => {
        match $rhs {
            Value::Component { ref ty, .. } => match ty.as_str() {
                "u32" => $expr!($lhs: $lhs_value, $lhs_ty; ref $rhs: U32, u32; $self; $op),
                "u64" => $expr!($lhs: $lhs_value, $lhs_ty; ref $rhs: U64, u64; $self; $op),
                "usize" => $expr!($lhs: $lhs_value, $lhs_ty; ref $rhs: Usize, usize; $self; $op),
                "i32" => $expr!($lhs: $lhs_value, $lhs_ty; ref $rhs: I32, i32; $self; $op),
                "i64" => $expr!($lhs: $lhs_value, $lhs_ty; ref $rhs: I64, i64; $self; $op),
                "f32" => $expr!($lhs: $lhs_value, $lhs_ty; ref $rhs: F32, f32; $self; $op),
                _ => {
                    todo!("Implement other values for rhs: {:?}", $rhs);
                }
            }
            _ => {
                todo!("Implement other values for rhs: {:?}", $rhs);
            }
        }
    };

    (ref $lhs:ident: $lhs_value:ident, $lhs_ty:ty ; ref $rhs:ident; $expr:ident; $self:ident ; $op:ident) => {
        match $rhs {
            Value::Component { ref ty, .. } => match ty.as_str() {
                "u32" => $expr!(ref $lhs: $lhs_value, $lhs_ty; ref $rhs: U32, u32; $self; $op),
                "u64" => $expr!(ref $lhs: $lhs_value, $lhs_ty; ref $rhs: U64, u64; $self; $op),
                "usize" => $expr!(ref $lhs: $lhs_value, $lhs_ty; ref $rhs: Usize, usize; $self; $op),
                "i32" => $expr!(ref $lhs: $lhs_value, $lhs_ty; ref $rhs: I32, i32; $self; $op),
                "i64" => $expr!(ref $lhs: $lhs_value, $lhs_ty; ref $rhs: I64, i64; $self; $op),
                "f32" => $expr!(ref $lhs: $lhs_value, $lhs_ty; ref $rhs: F32, f32; $self; $op),
                _ => {
                    todo!("Implement other values for rhs: {:?}", $rhs);
                }
            }
            _ => {
                todo!("Implement other values for rhs: {:?}", $rhs);
            }
        }
    };

    (ref mut $lhs:ident: $lhs_value:ident, $lhs_ty:ty ; ref $rhs:ident; $expr:ident; $self:ident ; $($op:ident)?) => {
        match $rhs {
            Value::Component { ref ty, .. } => match ty.as_str() {
                "u32" => $expr!(ref mut $lhs: $lhs_value, $lhs_ty; ref $rhs: U32, u32; $self; $($op)?),
                "u64" => $expr!(ref mut $lhs: $lhs_value, $lhs_ty; ref $rhs: U64, u64; $self; $($op)?),
                "usize" => $expr!(ref mut $lhs: $lhs_value, $lhs_ty; ref $rhs: Usize, usize; $self; $($op)?),
                "i32" => $expr!(ref mut $lhs: $lhs_value, $lhs_ty; ref $rhs: I32, i32; $self; $($op)?),
                "i64" => $expr!(ref mut $lhs: $lhs_value, $lhs_ty; ref $rhs: I64, i64; $self; $($op)?),
                "f32" => $expr!(ref mut $lhs: $lhs_value, $lhs_ty; ref $rhs: F32, f32; $self; $($op)?),
                _ => {
                    todo!("Implement other values for rhs: {:?}", $rhs);
                }
            }
            _ => {
                todo!("Implement other values for rhs: {:?}", $rhs);
            }
        }
    };
}

macro_rules! match_lhs_rhs {
    ($lhs:ident; $rhs:ident;  $expr:ident; $self:ident; $op:ident) => {
        match $lhs {
            Value::U32(lhs) => match_rhs!(lhs: U32, u32; $rhs; $expr; $self; $op),
            Value::U64(lhs) => match_rhs!(lhs: U64, u64; $rhs; $expr; $self; $op),
            Value::Usize(lhs) => match_rhs!(lhs: Usize, usize; $rhs; $expr; $self; $op),
            Value::I32(lhs) => match_rhs!(lhs: I32, i32; $rhs; $expr; $self; $op),
            Value::I64(lhs) => match_rhs!(lhs: I64, i64; $rhs; $expr; $self; $op),
            Value::F32(lhs) => match_rhs!(lhs: F32, f32; $rhs; $expr; $self; $op),
            Value::Component { .. } => match_lhs_rhs!(ref $lhs; $rhs; $expr; $self; $op),
            _ => {
                todo!("Implement other values for lhs: {:?}", $lhs);
            }
        }
    };

    (ref $lhs:ident; $rhs:ident; $expr:ident; $self:ident; $op:ident) => {
        match $lhs {
            Value::Component { ref ty, .. } => match ty.as_str() {
                "u32" => match_rhs!(ref $lhs: U32, u32; $rhs; $expr; $self; $op),
                "u64" => match_rhs!(ref $lhs: U64, u64; $rhs; $expr; $self; $op),
                "usize" => match_rhs!(ref $lhs: Usize, usize; $rhs; $expr; $self; $op),
                "i32" => match_rhs!(ref $lhs: I32, i32; $rhs; $expr; $self; $op),
                "i64" => match_rhs!(ref $lhs: I64, i64; $rhs; $expr; $self; $op),
                "f32" => match_rhs!(ref $lhs: F32, f32; $rhs; $expr; $self; $op),
                _ => {
                    todo!("Implement other values for lhs: {:?}", $lhs);
                }
            }
            _ => {
                todo!("Implement other values for lhs: {:?}", $lhs);
            }
        }
    };


    (ref mut $lhs:ident; $rhs:ident; $expr:ident; $self:ident; $($op:ident)?) => {
        match $lhs {
            Value::Component { ref ty, .. } => match ty.as_str() {

                "u32" => match_rhs!(ref mut $lhs: U32, u32; $rhs; $expr; $self; $($op)?),
                "u64" => match_rhs!(ref mut $lhs: U64, u64; $rhs; $expr; $self; $($op)?),
                "usize" => match_rhs!(ref mut $lhs: Usize, usize; $rhs; $expr; $self; $($op)?),
                "i32" => match_rhs!(ref mut $lhs: I32, i32; $rhs; $expr; $self; $($op)?),
                "i64" => match_rhs!(ref mut $lhs: I64, i64; $rhs; $expr; $self; $($op)?),
                "f32" => match_rhs!(ref mut $lhs: F32, f32; $rhs; $expr; $self; $($op)?),
                _ => {
                    todo!("Implement other values for lhs: {:?}", $lhs);
                }
            }
            _ => {
                todo!("Implement other values for lhs: {:?}", $lhs);
            }
        }
    };
}

macro_rules! value_arithmetic {
    // entry points
    ($lhs:ident $rhs:ident; $self:ident;) => {
        match_lhs_rhs!(ref mut $lhs ; $rhs ; value_arithmetic ; $self;)
    };
    ($lhs:ident $rhs:ident; $self:ident; $op:ident assign) => {
        match_lhs_rhs!(ref mut $lhs ; $rhs ; value_arithmetic ; $self; $op)
    };
    ($lhs:ident $rhs:ident; $self:ident; $op:ident) => {
        match_lhs_rhs!($lhs ; $rhs ; value_arithmetic ; $self; $op)
    };

    // ex: a += b
    (ref mut $lhs:ident : $lhs_value:ident, $lhs_ty:ty ; ref $rhs:ident: $rhs_value:ident, $rhs_ty:ty; $self:ident; $op:ident) => {{
        let rhs = match $rhs {
            Value::Component { ref name, .. } => name.clone(),
            _ => unreachable!(),
        };
        let rhs = $self.current_scope().query_params.get(&rhs).unwrap();
        let rhs = *rhs.get::<$rhs_ty>().unwrap();

        let lhs = match $lhs {
            Value::Component { ref name, .. } => name.clone(),
            _ => unreachable!(),
        };
        let lhs = $self.current_scope().query_params.get_mut(&lhs).unwrap();
        let lhs = lhs.get_mut::<$lhs_ty>().unwrap();

        *lhs = lhs.$op(rhs as $lhs_ty);

        Ok(Value::Void)
    }};

    // ex: a += 5
    (ref mut $lhs:ident : $lhs_value:ident, $lhs_ty:ty; $rhs:ident: $rhs_value:ident, $rhs_ty:ty ; $self:ident; $op:ident) => {{
        let lhs = match $lhs {
            Value::Component { ref name, .. } => name.clone(),
            _ => unreachable!(),
        };
        let lhs = $self.current_scope().query_params.get_mut(&lhs).unwrap();
        let lhs = lhs.get_mut::<$lhs_ty>().unwrap();

        *lhs = lhs.$op($rhs as $lhs_ty);

        Ok(Value::Void)
    }};

    // ex: a = b
    (ref mut $lhs:ident : $lhs_value:ident, $lhs_ty:ty ; ref $rhs:ident: $rhs_value:ident, $rhs_ty:ty; $self:ident;) => {{
        let rhs = match $rhs {
            Value::Component { ref name, .. } => name.clone(),
            _ => unreachable!(),
        };
        let rhs = $self.current_scope().query_params.get(&rhs).unwrap();
        let rhs = *rhs.get::<$rhs_ty>().unwrap();

        let lhs = match $lhs {
            Value::Component { ref name, .. } => name.clone(),
            _ => unreachable!(),
        };
        let lhs = $self.current_scope().query_params.get_mut(&lhs).unwrap();
        let lhs = lhs.get_mut::<$lhs_ty>().unwrap();

        *lhs = (rhs as $lhs_ty);

        Ok(Value::Void)
    }};

    // ex: a = 5
    (ref mut $lhs:ident : $lhs_value:ident, $lhs_ty:ty; $rhs:ident: $rhs_value:ident, $rhs_ty:ty ; $self:ident; ) => {{
        let lhs = match $lhs {
            Value::Component { ref name, .. } => name.clone(),
            _ => unreachable!(),
        };
        let lhs = $self.current_scope().query_params.get_mut(&lhs).unwrap();
        let lhs = lhs.get_mut::<$lhs_ty>().unwrap();

        *lhs = ($rhs as $lhs_ty);

        Ok(Value::Void)
    }};

    // ex: 4 + 5
    (    $lhs:ident : $lhs_value:ident, $lhs_ty:ty; $rhs:ident: $rhs_value:ident, $rhs_ty:ty ; $self:ident; $op:ident) => {{
        Ok(Value::$lhs_value($lhs.$op ($rhs as $lhs_ty)))
    }};

    // ex: 5 + b
    (    $lhs:ident : $lhs_value:ident, $lhs_ty:ty; ref $rhs:ident: $rhs_value:ident, $rhs_ty:ty; $self:ident; $op:ident) => {{
        value_arithmetic!(ref $rhs: $rhs_value, $rhs_ty ; $lhs: $lhs_value, $lhs_ty ; $self; $op)
    }};

    // ex: a + 5
    (ref $lhs:ident : $lhs_value:ident, $lhs_ty:ty; $rhs:ident: $rhs_value:ident, $rhs_ty:ty; $self:ident; $op:ident) => {{
        let lhs = match $lhs {
            Value::Component { ref name, .. } => name.clone(),
            _ => unreachable!(),
        };
        let lhs = $self.current_scope().query_params.get(&lhs).unwrap();
        let lhs = lhs.get::<$lhs_ty>().unwrap();

        Ok(Value::$lhs_value((lhs.$op ($rhs as $lhs_ty))))
    }};

    // ex: a + b
    (ref $lhs:ident : $lhs_value:ident, $lhs_ty:ty; ref $rhs:ident: $rhs_value:ident, $rhs_ty:ty; $self:ident; $op:ident) => {{

        let rhs = match $rhs {
            Value::Component { ref name, .. } => name.clone(),
            _ => unreachable!(),
        };
        let rhs = $self.current_scope().query_params.get(&rhs).unwrap();
        let rhs = *rhs.get::<$rhs_ty>().unwrap();

        let lhs = match $lhs {
            Value::Component { ref name, .. } => name.clone(),
            _ => unreachable!(),
        };
        let lhs = $self.current_scope().query_params.get(&lhs).unwrap();
        let lhs = lhs.get::<$lhs_ty>().unwrap();

        Ok(Value::$lhs_value((lhs.$op (rhs as $lhs_ty))))
    }};
}

pub struct InterpreterScope<'a> {
    pub values: FxHashMap<String, Value>,
    pub query_params: FxHashMap<String, DynamicQueryRef<'a>>,
}

pub struct InterpreterContext<'a, 'b> {
    pub world: Arc<RwLock<World>>,
    pub params: FxHashMap<String, &'b ScriptParam<'a>>,
    pub scopes: Vec<InterpreterScope<'a>>,
}

impl<'a, 'b> InterpreterContext<'a, 'b>
where
    'b: 'a,
{
    fn push_scope(&mut self) {
        if let Some(scope) = self.scopes.last_mut() {
            let new_scope = InterpreterScope {
                values: scope.values.clone(),
                query_params: scope.query_params.drain().collect(),
            };
            self.scopes.push(new_scope);
        } else {
            self.scopes.push(InterpreterScope {
                values: FxHashMap::default(),
                query_params: FxHashMap::default(),
            });
        }
    }

    fn pop_scope(&mut self) {
        let old_scope = self.scopes.pop();
        if let Some(old_scope) = old_scope {
            if let Some(current_scope) = self.scopes.last_mut() {
                current_scope.values.extend(old_scope.values);
                current_scope.query_params.extend(old_scope.query_params);
            }
        }
    }

    fn current_scope(&mut self) -> &mut InterpreterScope<'a> {
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
            Expr::FloatLiteral(float) => Ok(Value::F32(*float as f32)),
            Expr::IntLiteral(int) => Ok(Value::I64(*int)),
            Expr::StringLiteral(string) => Ok(Value::String(string.clone())),
            Expr::Block(block) => self.interp_block(&block.statements),
            Expr::Ident(ident) => {
                let param = self.current_scope().query_params.get(ident).unwrap();
                let ty = param.type_name();
                Ok(Value::Component {
                    name: ident.clone(),
                    ty: ty.to_string(),
                })
            }
            Expr::Prefix { op, rhs } => {
                let rhs = self.interp_expr(rhs)?;
                match op.as_str() {
                    "-" => match rhs {
                        Value::U32(rhs) => Ok(Value::I32(-(rhs as i32))),
                        Value::U64(rhs) => Ok(Value::I64(-(rhs as i64))),
                        Value::I32(rhs) => Ok(Value::I32(-rhs)),
                        Value::I64(rhs) => Ok(Value::I64(-rhs)),
                        Value::F32(rhs) => Ok(Value::F32(-rhs)),
                        _ => {
                            todo!("Implement other prefix operators");
                        }
                    },
                    _ => {
                        todo!("Implement other prefix operators");
                    }
                }
            }
            Expr::Infix { op, lhs, rhs } => {
                let lhs = self.interp_expr(lhs)?;
                let rhs = self.interp_expr(rhs)?;
                match op.as_str() {
                    "+" => value_arithmetic!(lhs rhs; self; add),
                    "-" => value_arithmetic!(lhs rhs; self; sub),
                    "*" => value_arithmetic!(lhs rhs; self; mul),
                    "/" => value_arithmetic!(lhs rhs; self; div),
                    "%" => value_arithmetic!(lhs rhs; self; rem),
                    "=" => value_arithmetic!(lhs rhs; self;),
                    "+=" => value_arithmetic!(lhs rhs; self; add assign),
                    "-=" => value_arithmetic!(lhs rhs; self; sub assign),
                    "*=" => value_arithmetic!(lhs rhs; self; mul assign),
                    "/=" => value_arithmetic!(lhs rhs; self; div assign),
                    "%=" => value_arithmetic!(lhs rhs; self; rem assign),
                    _ => {
                        todo!("Implement other infix operators");
                    }
                }
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
                            let param = self.current_scope().query_params.get(&name).unwrap();
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
                self.current_scope().values.insert(
                    ident.name.clone(),
                    Value::Component {
                        name: ident.name.clone(),
                        ty: ident.ty.clone(),
                    },
                );
                self.current_scope()
                    .query_params
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
                            scopes: Vec::new(),
                        };
                        ctx.interp_system(&system)
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
