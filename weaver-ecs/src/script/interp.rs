use std::{
    fmt::{Debug, Display},
    ops::Deref,
    sync::Arc,
};

use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use typed_arena::Arena;

use crate::{
    component::Data,
    id::DynamicId,
    prelude::World,
    query::{DynamicQueryParam, DynamicQueryParams, DynamicQueryRef},
    system::DynamicSystem,
};

use super::{
    parser::{Call, Expr, Query, Scope, Statement, System, TypedIdent},
    Script,
};

pub enum Value {
    Void,
    Float(f32),
    Int(i64),
    Data(Data),
    DataMut(Data),
}

impl Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Value::DataMut(data) = self {
            write!(f, "DataMut({:?})", data.type_name())
        } else if let Value::Data(data) = self {
            write!(f, "Data({:?})", data.type_name())
        } else {
            write!(f, "{:?}", self.type_name())
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Void => write!(f, "()"),
            Value::Float(float) => write!(f, "{}", float),
            Value::Int(int) => write!(f, "{}", int),
            Value::Data(data) => {
                data.display(f);
                Ok(())
            }
            Value::DataMut(data) => {
                data.display(f);
                Ok(())
            }
        }
    }
}

impl Value {
    pub fn type_name(&self) -> &str {
        match self {
            Value::Void => "()",
            Value::Float(_) => "f64",
            Value::Int(_) => "i64",
            Value::Data(data) => data.type_name(),
            Value::DataMut(data) => data.type_name(),
        }
    }

    pub fn infix(&self, op: &str, rhs: &Self) -> anyhow::Result<Self> {
        match (self, rhs) {
            (Value::Int(lhs), Value::Int(rhs)) => match op {
                "+" => Ok(Value::Int(lhs + rhs)),
                "-" => Ok(Value::Int(lhs - rhs)),
                "*" => Ok(Value::Int(lhs * rhs)),
                "/" => Ok(Value::Int(lhs / rhs)),
                "%" => Ok(Value::Int(lhs % rhs)),
                "=" | "+=" | "-=" | "*=" | "/=" | "%=" => {
                    Err(anyhow::anyhow!("Invalid operator {} for integer types", op))
                }
                _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
            },
            (Value::Float(lhs), Value::Float(rhs)) => match op {
                "+" => Ok(Value::Float(lhs + rhs)),
                "-" => Ok(Value::Float(lhs - rhs)),
                "*" => Ok(Value::Float(lhs * rhs)),
                "/" => Ok(Value::Float(lhs / rhs)),
                "%" => Ok(Value::Float(lhs % rhs)),
                "=" | "+=" | "-=" | "*=" | "/=" | "%=" => {
                    Err(anyhow::anyhow!("Invalid operator {} for float types", op))
                }
                _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
            },
            (Value::Data(lhs), Value::Data(rhs)) | (Value::Data(lhs), Value::DataMut(rhs)) => {
                match op {
                    "+" => Ok(Value::Data(lhs.add(rhs)?)),
                    "-" => Ok(Value::Data(lhs.sub(rhs)?)),
                    "*" => Ok(Value::Data(lhs.mul(rhs)?)),
                    "/" => Ok(Value::Data(lhs.div(rhs)?)),
                    "%" => Ok(Value::Data(lhs.rem(rhs)?)),
                    "=" | "+=" | "-=" | "*=" | "/=" | "%=" => Err(anyhow::anyhow!(
                        "Invalid operator {}: cannot assign to immutable variable {}",
                        op,
                        lhs.name()
                    )),
                    _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
                }
            }
            (Value::Data(lhs), Value::Int(rhs)) => {
                let rhs = Data::new(*rhs, None, lhs.registry());
                match op {
                    "+" => Ok(Value::DataMut(lhs.add(&rhs)?)),
                    "-" => Ok(Value::DataMut(lhs.sub(&rhs)?)),
                    "*" => Ok(Value::DataMut(lhs.mul(&rhs)?)),
                    "/" => Ok(Value::DataMut(lhs.div(&rhs)?)),
                    "%" => Ok(Value::DataMut(lhs.rem(&rhs)?)),
                    "=" | "+=" | "-=" | "*=" | "/=" | "%=" => Err(anyhow::anyhow!(
                        "Invalid operator {}: cannot assign to immutable variable {}",
                        op,
                        lhs.name()
                    )),
                    _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
                }
            }
            (Value::Int(lhs), Value::Data(rhs)) | (Value::Int(lhs), Value::DataMut(rhs)) => {
                let lhs = Data::new(*lhs, None, rhs.registry());
                match op {
                    "+" => Ok(Value::DataMut(lhs.add(rhs)?)),
                    "-" => Ok(Value::DataMut(lhs.sub(rhs)?)),
                    "*" => Ok(Value::DataMut(lhs.mul(rhs)?)),
                    "/" => Ok(Value::DataMut(lhs.div(rhs)?)),
                    "%" => Ok(Value::DataMut(lhs.rem(rhs)?)),
                    "=" | "+=" | "-=" | "*=" | "/=" | "%=" => {
                        Err(anyhow::anyhow!("Invalid operator {} for integer types", op,))
                    }
                    _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
                }
            }
            (Value::DataMut(lhs), Value::Int(rhs)) => {
                let rhs = Data::new(*rhs, None, lhs.registry());
                match op {
                    "+" => Ok(Value::DataMut(lhs.add(&rhs)?)),
                    "-" => Ok(Value::DataMut(lhs.sub(&rhs)?)),
                    "*" => Ok(Value::DataMut(lhs.mul(&rhs)?)),
                    "/" => Ok(Value::DataMut(lhs.div(&rhs)?)),
                    "%" => Ok(Value::DataMut(lhs.rem(&rhs)?)),
                    "=" => {
                        lhs.assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "+=" => {
                        lhs.add_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "-=" => {
                        lhs.sub_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "*=" => {
                        lhs.mul_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "/=" => {
                        lhs.div_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "%=" => {
                        lhs.rem_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
                }
            }
            (Value::Data(lhs), Value::Float(rhs)) => {
                let rhs = Data::new(*rhs, None, lhs.registry());
                match op {
                    "+" => Ok(Value::DataMut(lhs.add(&rhs)?)),
                    "-" => Ok(Value::DataMut(lhs.sub(&rhs)?)),
                    "*" => Ok(Value::DataMut(lhs.mul(&rhs)?)),
                    "/" => Ok(Value::DataMut(lhs.div(&rhs)?)),
                    "%" => Ok(Value::DataMut(lhs.rem(&rhs)?)),
                    "=" | "+=" | "-=" | "*=" | "/=" | "%=" => Err(anyhow::anyhow!(
                        "Invalid operator {}: cannot assign to immutable variable {}",
                        op,
                        lhs.name()
                    )),
                    _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
                }
            }
            (Value::Float(lhs), Value::Data(rhs)) | (Value::Float(lhs), Value::DataMut(rhs)) => {
                let lhs = Data::new(*lhs, None, rhs.registry());
                match op {
                    "+" => Ok(Value::DataMut(lhs.add(rhs)?)),
                    "-" => Ok(Value::DataMut(lhs.sub(rhs)?)),
                    "*" => Ok(Value::DataMut(lhs.mul(rhs)?)),
                    "/" => Ok(Value::DataMut(lhs.div(rhs)?)),
                    "%" => Ok(Value::DataMut(lhs.rem(rhs)?)),
                    "=" | "+=" | "-=" | "*=" | "/=" | "%=" => {
                        Err(anyhow::anyhow!("Invalid operator {} for float types", op,))
                    }
                    _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
                }
            }
            (Value::DataMut(lhs), Value::Float(rhs)) => {
                let rhs = Data::new(*rhs, None, lhs.registry());
                match op {
                    "+" => Ok(Value::DataMut(lhs.add(&rhs)?)),
                    "-" => Ok(Value::DataMut(lhs.sub(&rhs)?)),
                    "*" => Ok(Value::DataMut(lhs.mul(&rhs)?)),
                    "/" => Ok(Value::DataMut(lhs.div(&rhs)?)),
                    "%" => Ok(Value::DataMut(lhs.rem(&rhs)?)),
                    "=" => {
                        lhs.assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "+=" => {
                        lhs.add_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "-=" => {
                        lhs.sub_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "*=" => {
                        lhs.mul_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "/=" => {
                        lhs.div_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "%=" => {
                        lhs.rem_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
                }
            }
            (Value::DataMut(lhs), Value::Data(rhs))
            | (Value::DataMut(lhs), Value::DataMut(rhs)) => match op {
                "+" => Ok(Value::DataMut(lhs.add(rhs)?)),
                "-" => Ok(Value::DataMut(lhs.sub(rhs)?)),
                "*" => Ok(Value::DataMut(lhs.mul(rhs)?)),
                "/" => Ok(Value::DataMut(lhs.div(rhs)?)),
                "%" => Ok(Value::DataMut(lhs.rem(rhs)?)),
                "=" => {
                    lhs.assign(rhs)?;
                    Ok(Value::Void)
                }
                "+=" => {
                    lhs.add_assign(rhs)?;
                    Ok(Value::Void)
                }
                "-=" => {
                    lhs.sub_assign(rhs)?;
                    Ok(Value::Void)
                }
                "*=" => {
                    lhs.mul_assign(rhs)?;
                    Ok(Value::Void)
                }
                "/=" => {
                    lhs.div_assign(rhs)?;
                    Ok(Value::Void)
                }
                "%=" => {
                    lhs.rem_assign(rhs)?;
                    Ok(Value::Void)
                }
                _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
            },
            _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ValueHandle {
    pub name: Option<String>,
    pub id: usize,
    pub value: Arc<Value>,
}

impl Deref for ValueHandle {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        self.value.as_ref()
    }
}

pub struct RuntimeEnv {
    pub world: Arc<RwLock<World>>,
    pub arena: Arena<InterpreterContext>,
}

impl RuntimeEnv {
    pub fn new(world: Arc<RwLock<World>>) -> Self {
        Self {
            world,
            arena: Arena::new(),
        }
    }

    pub fn push_scope(&self, inherit: Option<&InterpreterContext>) -> &mut InterpreterContext {
        if let Some(inherit) = inherit {
            self.arena.alloc(inherit.clone())
        } else {
            self.arena.alloc(InterpreterContext::new())
        }
    }
}

#[derive(Clone)]
pub struct InterpreterContext {
    pub names: FxHashMap<String, usize>,
    pub heap: FxHashMap<usize, ValueHandle>,
}

impl InterpreterContext {
    pub fn new() -> Self {
        Self {
            names: FxHashMap::default(),
            heap: FxHashMap::default(),
        }
    }

    pub fn alloc_value(&mut self, name: Option<&str>, value: Arc<Value>) -> ValueHandle {
        let handle = ValueHandle {
            name: name.map(|s| s.to_owned()),
            id: self.heap.len(),
            value,
        };
        self.heap.insert(handle.id, handle.clone());
        if let Some(name) = name {
            self.names.insert(name.to_owned(), handle.id);
        }
        handle
    }

    pub fn get_value_handle(&self, name: &str) -> Option<ValueHandle> {
        if let Some(id) = self.names.get(name) {
            return self.heap.get(id).cloned();
        }
        None
    }

    pub fn interp_system(&mut self, env: &RuntimeEnv, system: &System) -> anyhow::Result<()> {
        for statement in &system.block.statements {
            self.interp_statement(env, statement)?;
        }
        Ok(())
    }

    pub fn interp_statement(
        &mut self,
        env: &RuntimeEnv,
        statement: &Statement,
    ) -> anyhow::Result<()> {
        match statement {
            Statement::Expr(expr) => {
                self.interp_expr(env, expr)?;
                Ok(())
            }
            Statement::Query(query) => {
                self.interp_query(env, query)?;
                Ok(())
            }
            _ => todo!("Implement other statements"),
        }
    }

    pub fn interp_expr(&mut self, env: &RuntimeEnv, expr: &Expr) -> anyhow::Result<ValueHandle> {
        match expr {
            Expr::Call(call) => self.interp_call(env, call),
            Expr::Ident(ident) => self.interp_ident(ident),
            Expr::IntLiteral(int) => self.interp_int_literal(*int),
            Expr::FloatLiteral(float) => self.interp_float_literal(*float as f32),
            Expr::Block(block) => self.interp_block(env, &block.statements),
            Expr::Infix { op, lhs, rhs } => self.interp_infix(env, op, lhs, rhs),
            Expr::Member { lhs, rhs } => self.interp_member(env, lhs, rhs),
            Expr::Decl {
                mutability,
                ident,
                initial_value,
            } => self.interp_decl(env, *mutability, ident, initial_value),
            _ => todo!("Implement other expressions"),
        }
    }

    pub fn interp_call(&mut self, env: &RuntimeEnv, call: &Call) -> anyhow::Result<ValueHandle> {
        match call.name.as_str() {
            "print" => {
                for arg in &call.args {
                    let value = self.interp_expr(env, arg)?;
                    print!("{} ", *value);
                }
                println!();
                Ok(self.alloc_value(None, Arc::new(Value::Void)))
            }
            _ => todo!("Implement other calls"),
        }
    }

    pub fn interp_ident(&mut self, ident: &str) -> anyhow::Result<ValueHandle> {
        if let Some(value) = self.get_value_handle(ident) {
            return Ok(value);
        }

        log::debug!(
            "Heap: {:?}",
            self.heap
                .values()
                .map(|k| k.name.as_ref())
                .collect::<Vec<_>>()
        );
        anyhow::bail!("Unknown identifier {}", ident)
    }

    pub fn interp_int_literal(&mut self, int: i64) -> anyhow::Result<ValueHandle> {
        Ok(self.alloc_value(None, Arc::new(Value::Int(int))))
    }

    pub fn interp_float_literal(&mut self, float: f32) -> anyhow::Result<ValueHandle> {
        Ok(self.alloc_value(None, Arc::new(Value::Float(float))))
    }

    pub fn interp_block(
        &mut self,
        env: &RuntimeEnv,
        block: &[Statement],
    ) -> anyhow::Result<ValueHandle> {
        let scope = env.push_scope(Some(self));
        for statement in block {
            scope.interp_statement(env, statement)?;
        }
        Ok(self.alloc_value(None, Arc::new(Value::Void)))
    }

    pub fn interp_infix(
        &mut self,
        env: &RuntimeEnv,
        op: &str,
        lhs: &Expr,
        rhs: &Expr,
    ) -> anyhow::Result<ValueHandle> {
        let lhs = self.interp_expr(env, lhs)?;
        let rhs = self.interp_expr(env, rhs)?;

        let result = lhs.infix(op, &rhs)?;
        Ok(self.alloc_value(None, Arc::new(result)))
    }

    pub fn interp_member(
        &mut self,
        env: &RuntimeEnv,
        lhs: &Expr,
        rhs: &Expr,
    ) -> anyhow::Result<ValueHandle> {
        let lhs = self.interp_expr(env, lhs)?;
        let rhs = match rhs {
            Expr::Ident(ident) => ident,
            _ => anyhow::bail!("Invalid member access"),
        };

        match lhs.value.as_ref() {
            Value::Data(data) => {
                let field = data.field_by_name(rhs).unwrap().to_owned();
                let value = Value::Data(field);
                let name = lhs
                    .name
                    .map(|s| format!("{}.{}", s, rhs))
                    .or_else(|| Some(rhs.to_owned()));
                Ok(self.alloc_value(name.as_deref(), Arc::new(value)))
            }
            Value::DataMut(data) => {
                let field = data.field_by_name(rhs).unwrap().to_owned();
                let value = Value::DataMut(field);
                let name = lhs
                    .name
                    .map(|s| format!("{}.{}", s, rhs))
                    .or_else(|| Some(rhs.to_owned()));
                Ok(self.alloc_value(name.as_deref(), Arc::new(value)))
            }
            _ => anyhow::bail!("Invalid member access"),
        }
    }

    pub fn interp_decl(
        &mut self,
        env: &RuntimeEnv,
        mutability: bool,
        ident: &str,
        initial_value: &Expr,
    ) -> anyhow::Result<ValueHandle> {
        let value = self.interp_expr(env, initial_value)?;
        let value = value.value.to_owned();
        let value = if mutability {
            match &*value {
                Value::Data(data) => Value::DataMut(data.to_owned()),
                Value::DataMut(data) => Value::DataMut(data.to_owned()),
                Value::Int(int) => {
                    Value::DataMut(Data::new(*int, None, env.world.read().registry()))
                }
                Value::Float(float) => {
                    Value::DataMut(Data::new(*float, None, env.world.read().registry()))
                }
                Value::Void => anyhow::bail!("Cannot assign void value"),
            }
        } else {
            match &*value {
                Value::Data(data) => Value::Data(data.to_owned()),
                Value::DataMut(data) => Value::Data(data.to_owned()),
                Value::Int(int) => Value::Data(Data::new(*int, None, env.world.read().registry())),
                Value::Float(float) => {
                    Value::Data(Data::new(*float, None, env.world.read().registry()))
                }
                Value::Void => anyhow::bail!("Cannot assign void value"),
            }
        };
        Ok(self.alloc_value(Some(ident), Arc::new(value)))
    }

    pub fn interp_query(&mut self, env: &RuntimeEnv, query: &Query) -> anyhow::Result<ValueHandle> {
        let params = query.build(env.world.clone())?;
        let world = env.world.read();
        let mut builder = world.query_dynamic();
        for param in params.params {
            match param {
                DynamicQueryParam::Read(id) => builder = builder.read_id(id),
                DynamicQueryParam::Write(id) => builder = builder.write_id(id),
                DynamicQueryParam::With(id) => builder = builder.with_id(id),
                DynamicQueryParam::Without(id) => builder = builder.without_id(id),
            }
        }
        let typed_idents = query.components.clone();
        let block = query.block.clone();
        let query = builder.build();

        for mut entries in query.iter() {
            let scope = env.push_scope(Some(self));
            for (entry, typed_ident) in entries.iter_mut().zip(typed_idents.iter()) {
                let name = typed_ident.name.clone();

                match entry {
                    DynamicQueryRef::Ref(data) => {
                        let value = Value::Data(data.to_owned());
                        scope.alloc_value(Some(name.as_str()), Arc::new(value));
                    }
                    DynamicQueryRef::Mut(data) => {
                        let value = Value::DataMut(data.to_owned());
                        scope.alloc_value(Some(name.as_str()), Arc::new(value));
                    }
                }
            }
            for statement in &block.statements {
                scope.interp_statement(env, statement)?;
            }
        }

        Ok(self.alloc_value(None, Arc::new(Value::Void)))
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
                _ => {
                    todo!("Implement other scopes");
                }
            }
        }

        Ok(systems)
    }
}

impl BuildOnWorld for System {
    type Output = DynamicSystem;

    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<DynamicSystem> {
        let script = DynamicSystem::script_builder(&self.name);

        let system = self.clone();
        let script = script.build(move |_| {
            let env = RuntimeEnv::new(world.clone());
            let ctx = env.push_scope(None);
            ctx.interp_system(&env, &system)?;
            Ok(())
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
