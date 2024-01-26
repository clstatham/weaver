use std::{error::Error, sync::Arc};

use parking_lot::RwLock;
use petgraph::stable_graph::NodeIndex;
use rustc_hash::FxHashMap;
use typed_arena::Arena;

use crate::{
    component::Data,
    prelude::{SystemStage, World},
    query::{DynamicQueryParam, DynamicQueryParams, DynamicQueryRef},
    registry::DynamicId,
    system::DynamicSystem,
};

use super::{
    parser::{Call, Expr, Query, Scope, Span, SpanExpr, Statement, System, TypedIdent},
    value::{Value, ValueHandle},
    Script,
};

pub struct RuntimeError {
    pub span: Span,
    pub message: String,
}

impl RuntimeError {
    pub fn new(span: Span, message: String) -> Self {
        Self { span, message }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "{}:{}\n{}",
            self.span.line_no,
            self.span.col_no,
            self.span.line.trim_end()
        )?;
        let spaces = " ".repeat(self.span.col_no - 1);
        writeln!(f, "{}^", spaces)?;
        writeln!(f, "{}", self.message)?;
        Ok(())
    }
}

impl std::fmt::Debug for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Error for RuntimeError {}

macro_rules! bail {
    ($span:expr, $message:expr) => {
        anyhow::bail!(runtime_error!($span, $message))
    };
}

macro_rules! runtime_error {
    ($span:expr, $message:expr) => {
        anyhow::anyhow!(RuntimeError::new($span.to_owned(), $message.to_owned()))
    };
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
    pub should_return: Option<Option<ValueHandle>>,
}

impl InterpreterContext {
    pub fn new() -> Self {
        Self {
            names: FxHashMap::default(),
            heap: FxHashMap::default(),
            should_break: None,
            should_return: None,
        }
    }

    pub fn alloc_value(&mut self, name: Option<&str>, value: Value) -> ValueHandle {
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
            Statement::Break(retval) => {
                let retval = if let Some(retval) = retval {
                    Some(self.interp_expr(env, retval)?)
                } else {
                    None
                };
                self.should_break = Some(retval);
                Ok(())
            }
            Statement::Return(retval) => {
                let retval = if let Some(retval) = retval {
                    Some(self.interp_expr(env, retval)?)
                } else {
                    None
                };
                self.should_return = Some(retval);
                Ok(())
            }
            _ => todo!("Implement other statements: {:?}", statement),
        }
    }

    pub fn interp_expr(
        &mut self,
        env: &RuntimeEnv,
        expr: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        match &expr.expr {
            Expr::Call(call) => self.interp_call(env, call),
            Expr::Ident(_) => self.interp_ident(expr),
            Expr::IntLiteral(int) => self.interp_int_literal(*int),
            Expr::FloatLiteral(float) => self.interp_float_literal(*float as f32),
            Expr::StringLiteral(string) => self.interp_string_literal(string),
            Expr::Block(block) => self.interp_block(env, &block.statements),
            Expr::Infix { op, lhs, rhs } => self.interp_infix(env, op, lhs, rhs),
            Expr::Member { lhs, rhs } => self.interp_member(env, lhs, rhs),
            Expr::Construct { name, args } => self.interp_construct(env, name, args),
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
            Expr::Query(query) => self.interp_query(env, query),
            Expr::Res {
                mutability,
                ident,
                res,
            } => self.interp_res(env, *mutability, ident, res),
            Expr::Type(typ) => Ok(self.alloc_value(None, Value::Type(typ.to_owned()))),
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
                Ok(self.alloc_value(None, Value::Void))
            }
            "spawn" => {
                let mut world = env.world.write();
                let entity = world.create_entity();
                drop(world);
                for arg in &call.args {
                    let component = self.interp_expr(env, arg)?;
                    let component = component.to_data(env)?;
                    let component = match component {
                        Value::Data(data) => data.to_owned(),
                        Value::DataMut(data) => data.to_owned(),
                        _ => bail!(arg.span, "Invalid argument"),
                    };

                    let mut world = env.world.write();
                    world.add_dynamic_component(&entity, component);
                }

                Ok(self.alloc_value(None, Value::Entity(entity)))
            }
            "vec3" => {
                if call.args.len() != 3 {
                    bail!(call.name.span, "Invalid argument count: expected 3");
                }

                let x = self.interp_expr(env, &call.args[0])?;
                let y = self.interp_expr(env, &call.args[1])?;
                let z = self.interp_expr(env, &call.args[2])?;

                let x = match &x.value {
                    Value::Int(int) => *int as f32,
                    Value::Float(float) => *float,
                    Value::Data(data) => *data.get_as::<f32>().ok_or(runtime_error!(
                        call.args[0].span,
                        "Invalid type: expected int or float"
                    ))?,
                    Value::DataMut(data) => *data.get_as::<f32>().ok_or(runtime_error!(
                        call.args[0].span,
                        "Invalid type: expected int or float"
                    ))?,
                    _ => bail!(call.args[0].span, "Invalid argument"),
                };

                let y = match &y.value {
                    Value::Int(int) => *int as f32,
                    Value::Float(float) => *float,
                    Value::Data(data) => *data.get_as::<f32>().ok_or(runtime_error!(
                        call.args[1].span,
                        "Invalid type: expected int or float"
                    ))?,
                    Value::DataMut(data) => *data.get_as::<f32>().ok_or(runtime_error!(
                        call.args[1].span,
                        "Invalid type: expected int or float"
                    ))?,
                    _ => bail!(call.args[1].span, "Invalid argument"),
                };

                let z = match &z.value {
                    Value::Int(int) => *int as f32,
                    Value::Float(float) => *float,
                    Value::Data(data) => *data.get_as::<f32>().ok_or(runtime_error!(
                        call.args[2].span,
                        "Invalid type: expected int or float"
                    ))?,
                    Value::DataMut(data) => *data.get_as::<f32>().ok_or(runtime_error!(
                        call.args[2].span,
                        "Invalid type: expected int or float"
                    ))?,
                    _ => bail!(call.args[2].span, "Invalid argument"),
                };

                let vec3 = glam::Vec3::new(x, y, z);

                Ok(self.alloc_value(None, Value::Vec3(vec3)))
            }
            "vec4" | "quat" => {
                if call.args.len() != 4 {
                    anyhow::bail!("Invalid argument count: expected 4");
                }

                let x = self.interp_expr(env, &call.args[0])?;
                let y = self.interp_expr(env, &call.args[1])?;
                let z = self.interp_expr(env, &call.args[2])?;
                let w = self.interp_expr(env, &call.args[3])?;

                let x = match &x.value {
                    Value::Int(int) => *int as f32,
                    Value::Float(float) => *float,
                    Value::Data(data) => *data.get_as::<f32>().ok_or(runtime_error!(
                        call.args[0].span,
                        "Invalid type: expected int or float"
                    ))?,
                    Value::DataMut(data) => *data.get_as::<f32>().ok_or(runtime_error!(
                        call.args[0].span,
                        "Invalid type: expected int or float"
                    ))?,
                    _ => bail!(call.args[0].span, "Invalid argument"),
                };

                let y = match &y.value {
                    Value::Int(int) => *int as f32,
                    Value::Float(float) => *float,
                    Value::Data(data) => *data.get_as::<f32>().ok_or(runtime_error!(
                        call.args[1].span,
                        "Invalid type: expected int or float"
                    ))?,
                    Value::DataMut(data) => *data.get_as::<f32>().ok_or(runtime_error!(
                        call.args[1].span,
                        "Invalid type: expected int or float"
                    ))?,
                    _ => bail!(call.args[1].span, "Invalid argument"),
                };

                let z = match &z.value {
                    Value::Int(int) => *int as f32,
                    Value::Float(float) => *float,
                    Value::Data(data) => *data.get_as::<f32>().ok_or(runtime_error!(
                        call.args[2].span,
                        "Invalid type: expected int or float"
                    ))?,
                    Value::DataMut(data) => *data.get_as::<f32>().ok_or(runtime_error!(
                        call.args[2].span,
                        "Invalid type: expected int or float"
                    ))?,
                    _ => bail!(call.args[2].span, "Invalid argument"),
                };

                let w = match &w.value {
                    Value::Int(int) => *int as f32,
                    Value::Float(float) => *float,
                    Value::Data(data) => *data.get_as::<f32>().ok_or(runtime_error!(
                        call.args[3].span,
                        "Invalid type: expected int or float"
                    ))?,
                    Value::DataMut(data) => *data.get_as::<f32>().ok_or(runtime_error!(
                        call.args[3].span,
                        "Invalid type: expected int or float"
                    ))?,
                    _ => bail!(call.args[3].span, "Invalid argument"),
                };

                let vec4 = glam::Vec4::new(x, y, z, w);

                Ok(self.alloc_value(None, Value::Vec4(vec4)))
            }
            "mat4" => {
                if call.args.len() != 16 {
                    anyhow::bail!("Invalid argument count");
                }

                let mut mat4 = Vec::new();

                for arg in &call.args[..16] {
                    let x = match &self.interp_expr(env, arg)?.value {
                        Value::Int(int) => *int as f32,
                        Value::Float(float) => *float,
                        Value::Data(data) => *data
                            .get_as::<f32>()
                            .ok_or(runtime_error!(arg.span, "Invalid type: expected f32"))?,
                        Value::DataMut(data) => *data
                            .get_as::<f32>()
                            .ok_or(runtime_error!(arg.span, "Invalid type: expected f32"))?,
                        _ => bail!(arg.span, "Invalid argument"),
                    };
                    mat4.push(x);
                }

                let mat4 = glam::Mat4::from_cols_slice(&mat4);

                Ok(self.alloc_value(None, Value::Mat4(mat4)))
            }
            name => {
                let func = env
                    .scopes
                    .iter()
                    .rev()
                    .filter_map(|scope| match scope {
                        Scope::Func(func) if func.name.as_str() == name => Some(func),
                        _ => None,
                    })
                    .next()
                    .ok_or(runtime_error!(call.name.span, "Unknown function"))?;

                let mut args = Vec::new();
                for (arg, param) in call.args.iter().zip(func.params.iter()) {
                    let mut value = self.interp_expr(env, arg)?;
                    value.name = Some(param.name.as_str().to_owned());
                    args.push(value);
                }

                let scope = env.push_scope(Some(self));
                for (arg, param) in args.iter().zip(func.params.iter()) {
                    let value = arg.value.to_data(env)?;
                    scope.alloc_value(Some(param.name.as_str()), value);
                }

                scope.interp_block(env, &func.block.statements)?;

                if let Some(should_break) = scope.should_break.take() {
                    if let Some(value) = should_break {
                        return Ok(value);
                    } else {
                        return Ok(self.alloc_value(None, Value::Void));
                    }
                }
                if let Some(should_return) = scope.should_return.take() {
                    if let Some(value) = should_return {
                        return Ok(value);
                    } else {
                        if func.ret_type.is_some() {
                            bail!(call.name.span, "Missing return value");
                        }
                        return Ok(self.alloc_value(None, Value::Void));
                    }
                }

                Ok(self.alloc_value(None, Value::Void))
            }
        }
    }

    pub fn interp_ident(&mut self, ident: &SpanExpr) -> anyhow::Result<ValueHandle> {
        if let Some(value) = self.get_value_handle(ident.as_str()) {
            return Ok(value);
        }

        log::debug!(
            "Heap: {:?}",
            self.heap
                .values()
                .map(|k| k.name.as_ref())
                .collect::<Vec<_>>()
        );
        bail!(ident.span, "Unknown identifier")
    }

    pub fn interp_int_literal(&mut self, int: i64) -> anyhow::Result<ValueHandle> {
        Ok(self.alloc_value(None, Value::Int(int)))
    }

    pub fn interp_float_literal(&mut self, float: f32) -> anyhow::Result<ValueHandle> {
        Ok(self.alloc_value(None, Value::Float(float)))
    }

    pub fn interp_string_literal(&mut self, string: &str) -> anyhow::Result<ValueHandle> {
        Ok(self.alloc_value(None, Value::String(string.to_owned())))
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
                    return Ok(self.alloc_value(None, Value::Void));
                }
            }
            if let Some(should_return) = scope.should_return.take() {
                self.should_return = Some(should_return.clone());
                if let Some(value) = should_return {
                    return Ok(value);
                } else {
                    return Ok(self.alloc_value(None, Value::Void));
                }
            }
            scope.interp_statement(env, statement)?;
        }
        self.should_break = scope.should_break.take();
        self.should_return = scope.should_return.take();
        Ok(self.alloc_value(None, Value::Void))
    }

    pub fn interp_infix(
        &mut self,
        env: &RuntimeEnv,
        op: &str,
        lhs: &SpanExpr,
        rhs: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        let mut lhs = self.interp_expr(env, lhs)?;
        let rhs = self.interp_expr(env, rhs)?;

        let result = lhs.infix(op, &rhs)?;
        Ok(self.alloc_value(None, result))
    }

    pub fn interp_member(
        &mut self,
        env: &RuntimeEnv,
        lhs: &SpanExpr,
        rhs: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        let lhs_value = self.interp_expr(env, lhs)?;

        match &rhs.expr {
            Expr::Ident(rhs_ident) => match &lhs_value.value {
                Value::Data(data) => {
                    let field = data
                        .field_by_name(rhs_ident)
                        .ok_or(runtime_error!(rhs.span, "No such field"))?
                        .to_owned();
                    let value = Value::Data(field);
                    let name = lhs_value
                        .name
                        .map(|s| format!("{}.{}", s, rhs_ident))
                        .or_else(|| Some(rhs_ident.to_owned()));
                    Ok(self.alloc_value(name.as_deref(), value))
                }
                Value::DataMut(data) => {
                    let field = data
                        .field_by_name(rhs_ident)
                        .ok_or(runtime_error!(rhs.span, "No such field"))?
                        .to_owned();
                    let value = Value::DataMut(field);
                    let name = lhs_value
                        .name
                        .map(|s| format!("{}.{}", s, rhs_ident))
                        .or_else(|| Some(rhs_ident.to_owned()));
                    Ok(self.alloc_value(name.as_deref(), value))
                }
                _ => bail!(rhs.span, "Invalid member access"),
            },
            Expr::Call(rhs_call) => {
                // look for an impl of the rhs call on the lhs type
                let func = env
                    .scopes
                    .iter()
                    .find_map(|scope| {
                        if let Scope::Impl(impl_) = scope {
                            Some(impl_.funcs.iter().find(|func| func.name == rhs_call.name))
                        } else {
                            None
                        }
                    })
                    .flatten();

                if let Some(func) = func {
                    let mut args = Vec::new();
                    for arg in &rhs_call.args {
                        args.push(self.interp_expr(env, arg)?);
                    }

                    let mut params = func.params.clone();

                    let scope = env.push_scope(None);
                    let mut has_self = false;
                    if let Some(first) = params.first() {
                        if first.name.as_str() == "self" {
                            if first.ty.as_str() != lhs_value.value.type_name()
                                && first.ty.as_str() != "Self"
                            {
                                bail!(first.ty.span, "Invalid self type")
                            }
                            let value = lhs_value.value.to_owned();
                            let value = value.to_data(env)?;
                            scope.alloc_value(Some(first.name.as_str()), value);
                            has_self = true;
                        }
                    }
                    if has_self {
                        params.remove(0);
                    }
                    for (arg, param) in args.iter().zip(params.iter()) {
                        if param.name.as_str() == "self" {
                            bail!(
                                param.name.span,
                                "Cannot use self as argument beyond the first"
                            );
                        } else {
                            let value = arg.value.to_owned();
                            let value = value.to_data(env)?;
                            scope.alloc_value(Some(param.name.as_str()), value);
                        }
                    }

                    scope.interp_block(env, &func.block.statements)?;

                    if let Some(should_break) = scope.should_break.take() {
                        if let Some(value) = should_break {
                            return Ok(value);
                        } else {
                            return Ok(self.alloc_value(None, Value::Void));
                        }
                    }
                    if let Some(should_return) = scope.should_return.take() {
                        return Ok(
                            should_return.unwrap_or_else(|| self.alloc_value(None, Value::Void))
                        );
                    }

                    Ok(self.alloc_value(None, Value::Void))
                } else {
                    let mut args = Vec::new();

                    // check if lhs is a type
                    if let Value::Type(lhs) = &lhs_value.value {
                        // get the method from the registry
                        let method = env
                            .world
                            .read()
                            .registry()
                            .method_by_name(lhs, rhs_call.name.as_str())
                            .ok_or(runtime_error!(rhs_call.name.span, "No such method"))?;

                        for arg in &rhs_call.args {
                            let arg = self.interp_expr(env, arg)?.to_data(env)?;
                            let arg = match arg {
                                Value::Data(data) => data,
                                Value::DataMut(data) => data,
                                _ => unreachable!(),
                            };
                            args.push(arg);
                        }

                        let args = args.iter().collect::<Vec<_>>();
                        let result = method.call(&args)?;
                        Ok(self.alloc_value(None, Value::Data(result)))
                    } else {
                        // look for a method on the lhs
                        let lhs_data = match lhs_value.value {
                            Value::Data(data) => data,
                            Value::DataMut(data) => data,
                            _ => bail!(lhs.span, "Invalid member access"),
                        };

                        // push a "self" value
                        args.push(lhs_data.to_owned());

                        for arg in &rhs_call.args {
                            let arg = self.interp_expr(env, arg)?.to_data(env)?;
                            let arg = match arg {
                                Value::Data(data) => data,
                                Value::DataMut(data) => data,
                                _ => unreachable!("to_data should only return Data or DataMut"),
                            };
                            args.push(arg);
                        }
                        let args = args.iter().collect::<Vec<_>>();
                        let result = lhs_data.call_method(rhs_call.name.as_str(), &args)?;
                        Ok(self.alloc_value(None, Value::Data(result)))
                    }
                }
            }
            _ => bail!(lhs.span, "Invalid member access"),
        }
    }

    pub fn interp_decl(
        &mut self,
        env: &RuntimeEnv,
        mutability: bool,
        ident: &SpanExpr,
        initial_value: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        let value = self.interp_expr(env, initial_value)?;
        let value = value.value.to_owned();
        let value = if mutability {
            match value {
                Value::Data(data) => Value::DataMut(data.to_owned()),
                Value::DataMut(data) => Value::DataMut(data.to_owned()),
                _ => value,
            }
        } else {
            match value {
                Value::Data(data) => Value::Data(data.to_owned()),
                Value::DataMut(data) => Value::Data(data.to_owned()),
                _ => value,
            }
        };
        Ok(self.alloc_value(Some(ident.as_str()), value))
    }

    pub fn interp_construct(
        &mut self,
        env: &RuntimeEnv,
        name: &SpanExpr,
        args: &[(String, SpanExpr)],
    ) -> anyhow::Result<ValueHandle> {
        let world = env.world.read();

        let mut is_clone = true; // todo?

        let mut fields = Vec::new();

        for (field_name, arg) in args {
            let value = self.interp_expr(env, arg)?;
            let value = value.value.to_owned();

            match &value {
                Value::Data(data) => {
                    if !data.is_clone() {
                        is_clone = false;
                    }
                    let mut data = data.to_owned();
                    data.field_name = Some(field_name.to_owned());
                    fields.push(data);
                }
                Value::DataMut(data) => {
                    if !data.is_clone() {
                        is_clone = false;
                    }
                    let mut data = data.to_owned();
                    data.field_name = Some(field_name.to_owned());
                    fields.push(data);
                }
                Value::Int(int) => {
                    let data = Data::new(*int, Some(field_name), world.registry());
                    fields.push(data);
                }
                Value::Float(float) => {
                    let data = Data::new(*float, Some(field_name), world.registry());
                    fields.push(data);
                }
                Value::Bool(b) => {
                    let data = Data::new(*b, Some(field_name), world.registry());
                    fields.push(data);
                }
                Value::Vec3(vec3) => {
                    let data = Data::new(*vec3, Some(field_name), world.registry());
                    fields.push(data);
                }
                Value::Vec4(vec4) => {
                    let data = Data::new(*vec4, Some(field_name), world.registry());
                    fields.push(data);
                }
                Value::Quat(quat) => {
                    let data = Data::new(*quat, Some(field_name), world.registry());
                    fields.push(data);
                }
                Value::Mat4(mat4) => {
                    let data = Data::new(*mat4, Some(field_name), world.registry());
                    fields.push(data);
                }
                Value::String(string) => {
                    let data = Data::new(string.to_owned(), Some(field_name), world.registry());
                    fields.push(data);
                }
                Value::Entity(_) => bail!(arg.span, "Cannot assign entity value"),
                Value::Void => bail!(arg.span, "Cannot assign void value"),
                Value::Query { .. } => bail!(arg.span, "Cannot assign query value"),
                Value::Type(_) => bail!(arg.span, "Cannot assign type value"),
            }
        }

        let component = Data::new_dynamic(name.as_str(), None, is_clone, fields, world.registry());

        Ok(self.alloc_value(None, Value::DataMut(component)))
    }

    pub fn interp_if(
        &mut self,
        env: &RuntimeEnv,
        condition: &SpanExpr,
        then_block: &SpanExpr,
        elif_blocks: &[(Box<SpanExpr>, Box<SpanExpr>)],
        else_block: Option<&SpanExpr>,
    ) -> anyhow::Result<ValueHandle> {
        let condition_value = self.interp_expr(env, condition)?;
        let condition = match &condition_value.value {
            Value::Int(int) => *int != 0,
            Value::Float(float) => *float != 0.0,
            Value::Bool(b) => *b,
            Value::Data(data) => *data
                .get_as::<bool>()
                .ok_or(runtime_error!(condition.span, "Invalid type for condition"))?,
            Value::DataMut(data) => *data
                .get_as::<bool>()
                .ok_or(runtime_error!(condition.span, "Invalid type for condition"))?,
            _ => bail!(condition.span, "Invalid type for condition"),
        };

        if condition {
            self.interp_expr(env, then_block)
        } else {
            for (condition, block) in elif_blocks {
                let condition_value = self.interp_expr(env, condition)?;
                let condition = match &condition_value.value {
                    Value::Int(int) => *int != 0,
                    Value::Float(float) => *float != 0.0,
                    Value::Bool(b) => *b,
                    Value::Data(data) => *data
                        .get_as::<bool>()
                        .ok_or(runtime_error!(condition.span, "Invalid type for condition"))?,
                    Value::DataMut(data) => *data
                        .get_as::<bool>()
                        .ok_or(runtime_error!(condition.span, "Invalid type for condition"))?,
                    _ => bail!(condition.span, "Invalid type for condition"),
                };

                if condition {
                    return self.interp_expr(env, block);
                }
            }

            if let Some(else_block) = else_block {
                self.interp_expr(env, else_block)
            } else {
                Ok(self.alloc_value(None, Value::Void))
            }
        }
    }

    pub fn interp_loop(
        &mut self,
        env: &RuntimeEnv,
        condition: Option<&SpanExpr>,
        block: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        if let Some(SpanExpr {
            expr: Expr::Ident(_),
            ..
        }) = condition
        {
            // named query
            let query = self.interp_ident(condition.as_ref().unwrap())?;
            let (query, typed_idents) = match &query.value {
                Value::Query {
                    query,
                    typed_idents,
                } => (query, typed_idents),
                _ => bail!(condition.unwrap().span, "Invalid query"),
            };

            for entries in query.iter() {
                let scope = env.push_scope(Some(self));
                for (entry, typed_ident) in entries.iter().zip(typed_idents.iter()) {
                    let name = typed_ident.name.clone();

                    match entry {
                        DynamicQueryRef::Ref(data) => {
                            let value = Value::Data(data.to_owned());
                            scope.alloc_value(Some(name.as_str()), value);
                        }
                        DynamicQueryRef::Mut(data) => {
                            let value = Value::DataMut(data.to_owned());
                            scope.alloc_value(Some(name.as_str()), value);
                        }
                    }
                }
                scope.interp_expr(env, block)?;

                if let Some(should_break) = scope.should_break.take() {
                    if let Some(value) = should_break {
                        return Ok(value);
                    } else {
                        return Ok(self.alloc_value(None, Value::Void));
                    }
                }
                if let Some(should_return) = scope.should_return.take() {
                    return Ok(should_return.unwrap_or_else(|| self.alloc_value(None, Value::Void)));
                }
            }

            Ok(self.alloc_value(None, Value::Void))
        } else if let Some(SpanExpr {
            expr: Expr::Query(query),
            ..
        }) = condition
        {
            // inline query
            let query = self.interp_query(env, query)?;
            let (query, typed_idents) = match &query.value {
                Value::Query {
                    query,
                    typed_idents,
                } => (query, typed_idents),
                _ => bail!(condition.unwrap().span, "Invalid query"),
            };

            for entries in query.iter() {
                let scope = env.push_scope(Some(self));
                for (entry, typed_ident) in entries.iter().zip(typed_idents.iter()) {
                    let name = typed_ident.name.clone();

                    match entry {
                        DynamicQueryRef::Ref(data) => {
                            let value = Value::Data(data.to_owned());
                            scope.alloc_value(Some(name.as_str()), value);
                        }
                        DynamicQueryRef::Mut(data) => {
                            let value = Value::DataMut(data.to_owned());
                            scope.alloc_value(Some(name.as_str()), value);
                        }
                    }
                }
                scope.interp_expr(env, block)?;

                if let Some(should_break) = scope.should_break.take() {
                    if let Some(value) = should_break {
                        return Ok(value);
                    } else {
                        return Ok(self.alloc_value(None, Value::Void));
                    }
                }
                if let Some(should_return) = scope.should_return.take() {
                    return Ok(should_return.unwrap_or_else(|| self.alloc_value(None, Value::Void)));
                }
            }

            Ok(self.alloc_value(None, Value::Void))
        } else {
            loop {
                if let Some(condition) = condition {
                    let condition_value = self.interp_expr(env, condition)?;
                    let condition = match &condition_value.value {
                        Value::Int(int) => *int != 0,
                        Value::Float(float) => *float != 0.0,
                        Value::Bool(b) => *b,
                        Value::Data(data) => *data
                            .get_as::<bool>()
                            .ok_or(runtime_error!(condition.span, "Invalid type for condition"))?,
                        Value::DataMut(data) => *data
                            .get_as::<bool>()
                            .ok_or(runtime_error!(condition.span, "Invalid type for condition"))?,
                        Value::Query { .. } => unreachable!(),
                        _ => bail!(condition.span, "Invalid type for condition"),
                    };

                    if !condition {
                        return Ok(self.alloc_value(None, Value::Void));
                    }
                }

                self.interp_expr(env, block)?;

                if let Some(should_break) = self.should_break.take() {
                    if let Some(value) = should_break {
                        return Ok(value);
                    } else {
                        return Ok(self.alloc_value(None, Value::Void));
                    }
                }
                if let Some(should_return) = self.should_return.take() {
                    return Ok(should_return.unwrap_or_else(|| self.alloc_value(None, Value::Void)));
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
        let name = query.name.as_str();
        let typed_idents = query.components.to_owned();
        let query = builder.build();

        Ok(self.alloc_value(
            Some(name),
            Value::Query {
                query,
                typed_idents,
            },
        ))
    }

    pub fn interp_res(
        &mut self,
        env: &RuntimeEnv,
        mutability: bool,
        ident: &SpanExpr,
        res: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        let world = env.world.read();
        let id = world.named_id(res.as_str());
        let data = world
            .dynamic_resource(id)
            .map_err(|_| runtime_error!(res.span, "No such resource"))?
            .to_owned();
        let value = if mutability {
            Value::DataMut(data)
        } else {
            Value::Data(data)
        };
        Ok(self.alloc_value(Some(ident.as_str()), value))
    }
}

impl Default for InterpreterContext {
    fn default() -> Self {
        Self::new()
    }
}

pub trait BuildOnWorld {
    type Output;

    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<Self::Output>;
}

impl BuildOnWorld for Script {
    type Output = Vec<(SystemStage, NodeIndex)>;

    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<Vec<(SystemStage, NodeIndex)>> {
        let mut nodes = Vec::new();
        for scope in &self.scopes {
            match scope {
                Scope::System(ref system) => {
                    let system_clone = system.clone();
                    let scopes = self.scopes.clone();
                    let world_clone = world.clone();
                    let run_fn = move || {
                        let env = RuntimeEnv::new(world_clone.clone(), scopes.clone());
                        let ctx = env.push_scope(None);
                        ctx.interp_system(&env, &system_clone)?;
                        Ok(())
                    };

                    let script = DynamicSystem::new(
                        system.name.as_str(),
                        vec![],
                        vec![],
                        vec![],
                        vec![],
                        run_fn,
                    );

                    if let Some(tag) = &system.tag {
                        match tag.as_str() {
                            "@startup" => {
                                nodes.push((
                                    SystemStage::Startup,
                                    world
                                        .read()
                                        .add_dynamic_system_to_stage(script, SystemStage::Startup),
                                ));
                            }
                            "@update" => {
                                nodes.push((
                                    SystemStage::Update,
                                    world
                                        .read()
                                        .add_dynamic_system_to_stage(script, SystemStage::Update),
                                ));
                            }
                            _ => todo!("Implement other tags"),
                        }
                    } else {
                        nodes.push((
                            SystemStage::Update,
                            world
                                .read()
                                .add_dynamic_system_to_stage(script, SystemStage::Update),
                        ));
                    }
                }
                Scope::Component(_component) => {
                    todo!("Implement component registration")
                }
                Scope::Func(_) => {}
                Scope::Impl(_) => {}
                Scope::Program(_) => unreachable!(),
            }
        }

        Ok(nodes)
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
            let id = with.as_str().to_string().build(world.clone())?;
            params = params.with(id);
        }
        for without in &self.without {
            let id = without.as_str().to_string().build(world.clone())?;
            params = params.without(id);
        }
        Ok(params)
    }
}

impl BuildOnWorld for TypedIdent {
    type Output = DynamicId;

    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<DynamicId> {
        Ok(world.read().named_id(self.ty.as_str()))
    }
}

impl BuildOnWorld for String {
    type Output = DynamicId;

    fn build(&self, world: Arc<RwLock<World>>) -> anyhow::Result<DynamicId> {
        Ok(world.read().named_id(self))
    }
}
