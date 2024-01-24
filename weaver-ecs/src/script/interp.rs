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
    prelude::{Entity, SystemStage, World},
    query::{DynamicQueryParam, DynamicQueryParams, DynamicQueryRef},
    registry::DynamicId,
    system::DynamicSystem,
};

use super::{
    parser::{Call, Component, Expr, Query, Scope, Statement, System, TypedIdent},
    Script,
};

pub enum Value {
    Void,
    Float(f32),
    Int(i64),
    Bool(bool),
    Data(Data),
    DataMut(Data),
    Entity(Entity),
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
            Value::Bool(bool) => write!(f, "{}", bool),
            Value::Data(data) => {
                data.display(f);
                Ok(())
            }
            Value::DataMut(data) => {
                data.display(f);
                Ok(())
            }
            Value::Entity(entity) => write!(f, "{:?}", entity),
        }
    }
}

impl Value {
    pub fn type_name(&self) -> &str {
        match self {
            Value::Void => "()",
            Value::Float(_) => "f64",
            Value::Int(_) => "i64",
            Value::Bool(_) => "bool",
            Value::Data(data) => data.type_name(),
            Value::DataMut(data) => data.type_name(),
            Value::Entity(_) => "Entity",
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
                ">" => Ok(Value::Bool(lhs > rhs)),
                "<" => Ok(Value::Bool(lhs < rhs)),
                ">=" => Ok(Value::Bool(lhs >= rhs)),
                "<=" => Ok(Value::Bool(lhs <= rhs)),
                "==" => Ok(Value::Bool(lhs == rhs)),
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
                ">" => Ok(Value::Bool(lhs > rhs)),
                "<" => Ok(Value::Bool(lhs < rhs)),
                ">=" => Ok(Value::Bool(lhs >= rhs)),
                "<=" => Ok(Value::Bool(lhs <= rhs)),
                "==" => Ok(Value::Bool(lhs == rhs)),
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
                    ">" => Ok(Value::Data(lhs.gt(rhs)?)),
                    "<" => Ok(Value::Data(lhs.lt(rhs)?)),
                    ">=" => Ok(Value::Data(lhs.ge(rhs)?)),
                    "<=" => Ok(Value::Data(lhs.le(rhs)?)),
                    "==" => Ok(Value::Data(lhs.eq(rhs)?)),
                    "&&" => Ok(Value::Data(lhs.and(rhs)?)),
                    "||" => Ok(Value::Data(lhs.or(rhs)?)),
                    "^" => Ok(Value::Data(lhs.xor(rhs)?)),
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
                    ">" => Ok(Value::DataMut(lhs.gt(&rhs)?)),
                    "<" => Ok(Value::DataMut(lhs.lt(&rhs)?)),
                    ">=" => Ok(Value::DataMut(lhs.ge(&rhs)?)),
                    "<=" => Ok(Value::DataMut(lhs.le(&rhs)?)),
                    "==" => Ok(Value::DataMut(lhs.eq(&rhs)?)),
                    "&&" => Ok(Value::DataMut(lhs.and(&rhs)?)),
                    "||" => Ok(Value::DataMut(lhs.or(&rhs)?)),
                    "^" => Ok(Value::DataMut(lhs.xor(&rhs)?)),
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
                    ">" => Ok(Value::DataMut(lhs.gt(rhs)?)),
                    "<" => Ok(Value::DataMut(lhs.lt(rhs)?)),
                    ">=" => Ok(Value::DataMut(lhs.ge(rhs)?)),
                    "<=" => Ok(Value::DataMut(lhs.le(rhs)?)),
                    "==" => Ok(Value::DataMut(lhs.eq(rhs)?)),
                    "&&" => Ok(Value::DataMut(lhs.and(rhs)?)),
                    "||" => Ok(Value::DataMut(lhs.or(rhs)?)),
                    "^" => Ok(Value::DataMut(lhs.xor(rhs)?)),
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
                    ">" => Ok(Value::DataMut(lhs.gt(&rhs)?)),
                    "<" => Ok(Value::DataMut(lhs.lt(&rhs)?)),
                    ">=" => Ok(Value::DataMut(lhs.ge(&rhs)?)),
                    "<=" => Ok(Value::DataMut(lhs.le(&rhs)?)),
                    "==" => Ok(Value::DataMut(lhs.eq(&rhs)?)),
                    "&&" => Ok(Value::DataMut(lhs.and(&rhs)?)),
                    "||" => Ok(Value::DataMut(lhs.or(&rhs)?)),
                    "^" => Ok(Value::DataMut(lhs.xor(&rhs)?)),
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
                    ">" => Ok(Value::DataMut(lhs.gt(&rhs)?)),
                    "<" => Ok(Value::DataMut(lhs.lt(&rhs)?)),
                    ">=" => Ok(Value::DataMut(lhs.ge(&rhs)?)),
                    "<=" => Ok(Value::DataMut(lhs.le(&rhs)?)),
                    "==" => Ok(Value::DataMut(lhs.eq(&rhs)?)),
                    "&&" => Ok(Value::DataMut(lhs.and(&rhs)?)),
                    "||" => Ok(Value::DataMut(lhs.or(&rhs)?)),
                    "^" => Ok(Value::DataMut(lhs.xor(&rhs)?)),
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
                    ">" => Ok(Value::DataMut(lhs.gt(rhs)?)),
                    "<" => Ok(Value::DataMut(lhs.lt(rhs)?)),
                    ">=" => Ok(Value::DataMut(lhs.ge(rhs)?)),
                    "<=" => Ok(Value::DataMut(lhs.le(rhs)?)),
                    "==" => Ok(Value::DataMut(lhs.eq(rhs)?)),
                    "&&" => Ok(Value::DataMut(lhs.and(rhs)?)),
                    "||" => Ok(Value::DataMut(lhs.or(rhs)?)),
                    "^" => Ok(Value::DataMut(lhs.xor(rhs)?)),
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
                    ">" => Ok(Value::DataMut(lhs.gt(&rhs)?)),
                    "<" => Ok(Value::DataMut(lhs.lt(&rhs)?)),
                    ">=" => Ok(Value::DataMut(lhs.ge(&rhs)?)),
                    "<=" => Ok(Value::DataMut(lhs.le(&rhs)?)),
                    "==" => Ok(Value::DataMut(lhs.eq(&rhs)?)),
                    "&&" => Ok(Value::DataMut(lhs.and(&rhs)?)),
                    "||" => Ok(Value::DataMut(lhs.or(&rhs)?)),
                    "^" => Ok(Value::DataMut(lhs.xor(&rhs)?)),
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
                ">" => Ok(Value::DataMut(lhs.gt(rhs)?)),
                "<" => Ok(Value::DataMut(lhs.lt(rhs)?)),
                ">=" => Ok(Value::DataMut(lhs.ge(rhs)?)),
                "<=" => Ok(Value::DataMut(lhs.le(rhs)?)),
                "==" => Ok(Value::DataMut(lhs.eq(rhs)?)),
                "&&" => Ok(Value::DataMut(lhs.and(rhs)?)),
                "||" => Ok(Value::DataMut(lhs.or(rhs)?)),
                "^" => Ok(Value::DataMut(lhs.xor(rhs)?)),
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
    pub scopes: Vec<Scope>,
    pub arena: Arena<InterpreterContext>,
}

impl RuntimeEnv {
    pub fn new(world: Arc<RwLock<World>>, scopes: Vec<Scope>) -> Self {
        Self {
            world,
            scopes,
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
    pub should_break: Option<Option<ValueHandle>>,
}

impl InterpreterContext {
    pub fn new() -> Self {
        Self {
            names: FxHashMap::default(),
            heap: FxHashMap::default(),
            should_break: None,
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
        self.interp_block(env, &system.block.statements)?;
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
            Statement::Break(retval) => {
                let retval = if let Some(retval) = retval {
                    Some(self.interp_expr(env, retval)?)
                } else {
                    None
                };
                self.should_break = Some(retval);
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
            Expr::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => self.interp_if(
                env,
                condition,
                then_block,
                elif_blocks,
                else_block.as_deref(),
            ),
            Expr::Loop { condition, block } => self.interp_loop(env, condition.as_deref(), block),
            _ => todo!("Implement other expressions: {:?}", expr),
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
            "spawn" => {
                let component = match &call.args[0] {
                    Expr::Construct { name, args } => self.interp_construct(env, name, args)?,
                    _ => anyhow::bail!("Invalid component"),
                };

                Ok(component)
            }
            name => {
                let func = env
                    .scopes
                    .iter()
                    .rev()
                    .filter_map(|scope| match scope {
                        Scope::Func(func) if func.name == name => Some(func),
                        _ => None,
                    })
                    .next()
                    .ok_or(anyhow::anyhow!("Unknown function {}", name))?;

                let mut args = Vec::new();
                for arg in &call.args {
                    args.push(self.interp_expr(env, arg)?);
                }

                let scope = env.push_scope(Some(self));
                for (arg, param) in args.iter().zip(func.params.iter()) {
                    let value = arg.value.to_owned();
                    let value = match &*value {
                        // pass by reference
                        Value::Data(data) => Value::Data(data.to_owned()),
                        Value::DataMut(data) => Value::DataMut(data.to_owned()),
                        Value::Int(int) => {
                            Value::Data(Data::new(*int, None, env.world.read().registry()))
                        }
                        Value::Float(float) => {
                            Value::Data(Data::new(*float, None, env.world.read().registry()))
                        }
                        Value::Bool(b) => {
                            Value::Data(Data::new(*b, None, env.world.read().registry()))
                        }
                        Value::Void => anyhow::bail!("Cannot assign void value"),
                        Value::Entity(entity) => Value::Entity(*entity),
                    };
                    scope.alloc_value(Some(param.name.as_str()), Arc::new(value));
                }

                scope.interp_block(env, &func.block.statements)?;

                if let Some(should_break) = scope.should_break.take() {
                    if let Some(value) = should_break {
                        return Ok(value);
                    } else {
                        return Ok(self.alloc_value(None, Arc::new(Value::Void)));
                    }
                }

                Ok(self.alloc_value(None, Arc::new(Value::Void)))
            }
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
            if let Some(should_break) = scope.should_break.take() {
                self.should_break = Some(should_break.clone());
                if let Some(value) = should_break {
                    return Ok(value);
                } else {
                    return Ok(self.alloc_value(None, Arc::new(Value::Void)));
                }
            }
            scope.interp_statement(env, statement)?;
        }
        self.should_break = scope.should_break.take();
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
                Value::Bool(b) => Value::DataMut(Data::new(*b, None, env.world.read().registry())),
                Value::Void => anyhow::bail!("Cannot assign void value"),
                Value::Entity(entity) => Value::Entity(*entity),
            }
        } else {
            match &*value {
                Value::Data(data) => Value::Data(data.to_owned()),
                Value::DataMut(data) => Value::Data(data.to_owned()),
                Value::Int(int) => Value::Data(Data::new(*int, None, env.world.read().registry())),
                Value::Float(float) => {
                    Value::Data(Data::new(*float, None, env.world.read().registry()))
                }
                Value::Bool(b) => Value::Data(Data::new(*b, None, env.world.read().registry())),
                Value::Void => anyhow::bail!("Cannot assign void value"),
                Value::Entity(entity) => Value::Entity(*entity),
            }
        };
        Ok(self.alloc_value(Some(ident), Arc::new(value)))
    }

    pub fn interp_construct(
        &mut self,
        env: &RuntimeEnv,
        name: &str,
        args: &[Expr],
    ) -> anyhow::Result<ValueHandle> {
        let mut world = env.world.write();

        let entity = world.components.create_entity();

        let mut fields = Vec::new();

        for arg in args {
            let (value, field_name) = if let Expr::Infix { op, lhs, rhs } = arg {
                if op != "=" {
                    anyhow::bail!("Invalid constructor argument");
                }
                let field_name = match lhs.as_ref() {
                    Expr::Ident(ident) => ident,
                    _ => anyhow::bail!("Invalid constructor argument"),
                }
                .to_owned();
                let value = self.interp_expr(env, rhs)?;
                (value, Some(field_name))
            } else {
                anyhow::bail!("Invalid constructor argument");
            };
            let value = value.value.to_owned();

            match &*value {
                Value::Data(data) => {
                    // fields.push(data.to_owned());
                    let mut data = data.try_clone().unwrap();
                    data.field_name = field_name;
                    fields.push(data);
                }
                Value::DataMut(data) => {
                    // fields.push(data.to_owned());
                    let mut data = data.try_clone().unwrap();
                    data.field_name = field_name;
                    fields.push(data);
                }
                Value::Int(int) => {
                    let data = Data::new(*int, field_name.as_deref(), world.registry());
                    fields.push(data);
                }
                Value::Float(float) => {
                    let data = Data::new(*float, field_name.as_deref(), world.registry());
                    fields.push(data);
                }
                Value::Bool(b) => {
                    let data = Data::new(*b, field_name.as_deref(), world.registry());
                    fields.push(data);
                }
                Value::Entity(_) => anyhow::bail!("Cannot assign entity value"),
                Value::Void => anyhow::bail!("Cannot assign void value"),
            }
        }

        let component = Data::new_dynamic(name, None, fields, world.registry());

        world.components.add_dynamic_component(&entity, component);

        Ok(self.alloc_value(None, Arc::new(Value::Entity(entity))))
    }

    pub fn interp_if(
        &mut self,
        env: &RuntimeEnv,
        condition: &Expr,
        then_block: &Expr,
        elif_blocks: &[(Box<Expr>, Box<Expr>)],
        else_block: Option<&Expr>,
    ) -> anyhow::Result<ValueHandle> {
        let condition = self.interp_expr(env, condition)?;
        let condition = match condition.value.as_ref() {
            Value::Int(int) => *int != 0,
            Value::Float(float) => *float != 0.0,
            Value::Bool(b) => *b,
            Value::Data(data) => *data.get_as::<bool>(),
            Value::DataMut(data) => *data.get_as::<bool>(),
            Value::Void => anyhow::bail!("Cannot use void value as condition"),
            Value::Entity(_) => anyhow::bail!("Cannot use entity value as condition"),
        };

        if condition {
            self.interp_expr(env, then_block)
        } else {
            for (condition, block) in elif_blocks {
                let condition = self.interp_expr(env, condition)?;
                let condition = match condition.value.as_ref() {
                    Value::Int(int) => *int != 0,
                    Value::Float(float) => *float != 0.0,
                    Value::Bool(b) => *b,
                    Value::Data(data) => *data.get_as::<bool>(),
                    Value::DataMut(data) => *data.get_as::<bool>(),
                    Value::Void => anyhow::bail!("Cannot use void value as condition"),
                    Value::Entity(_) => anyhow::bail!("Cannot use entity value as condition"),
                };

                if condition {
                    return self.interp_expr(env, block);
                }
            }

            if let Some(else_block) = else_block {
                self.interp_expr(env, else_block)
            } else {
                Ok(self.alloc_value(None, Arc::new(Value::Void)))
            }
        }
    }

    pub fn interp_loop(
        &mut self,
        env: &RuntimeEnv,
        condition: Option<&Expr>,
        block: &Expr,
    ) -> anyhow::Result<ValueHandle> {
        loop {
            if let Some(condition) = condition {
                let condition = self.interp_expr(env, condition)?;
                let condition = match condition.value.as_ref() {
                    Value::Int(int) => *int != 0,
                    Value::Float(float) => *float != 0.0,
                    Value::Bool(b) => *b,
                    Value::Data(data) => *data.get_as::<bool>(),
                    Value::DataMut(data) => *data.get_as::<bool>(),
                    Value::Void => anyhow::bail!("Cannot use void value as condition"),
                    Value::Entity(_) => anyhow::bail!("Cannot use entity value as condition"),
                };

                if !condition {
                    return Ok(self.alloc_value(None, Arc::new(Value::Void)));
                }
            }

            self.interp_expr(env, block)?;

            if let Some(should_break) = self.should_break.take() {
                if let Some(value) = should_break {
                    return Ok(value);
                } else {
                    return Ok(self.alloc_value(None, Arc::new(Value::Void)));
                }
            }
        }
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
            scope.interp_block(env, &block.statements)?;

            if let Some(should_break) = scope.should_break.take() {
                if let Some(value) = should_break {
                    return Ok(value);
                } else {
                    return Ok(self.alloc_value(None, Arc::new(Value::Void)));
                }
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
    type Output = ();

    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<()> {
        for scope in &self.scopes {
            match scope {
                Scope::System(ref system) => {
                    let script = DynamicSystem::script_builder(&self.name);

                    let system_clone = system.clone();
                    let scopes = self.scopes.clone();
                    let world_clone = world.clone();
                    let script = script.build(move || {
                        let env = RuntimeEnv::new(world_clone.clone(), scopes.clone());
                        let ctx = env.push_scope(None);
                        ctx.interp_system(&env, &system_clone)?;
                        Ok(())
                    });

                    if let Some(tag) = &system.tag {
                        match tag.as_str() {
                            "@startup" => {
                                world
                                    .write()
                                    .add_dynamic_system_to_stage(script, SystemStage::Startup);
                            }
                            "@update" => {
                                world
                                    .write()
                                    .add_dynamic_system_to_stage(script, SystemStage::Update);
                            }
                            _ => todo!("Implement other tags"),
                        }
                    } else {
                        world
                            .write()
                            .add_dynamic_system_to_stage(script, SystemStage::Update);
                    }
                }
                Scope::Component(ref component) => {
                    component.build(world.clone())?;
                }
                Scope::Func(_) => {}
                Scope::Program(_) => unreachable!(),
            }
        }

        Ok(())
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

        Ok(builder.build())
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
