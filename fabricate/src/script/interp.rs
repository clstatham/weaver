use std::error::Error;

use anyhow::ensure;
use rustc_hash::FxHashMap;
use typed_arena::Arena;

use crate::{
    component::{MethodArg, TakesSelf},
    prelude::{Data, Entity, LockedWorldHandle, SharedLock},
    query::{QueryBuilderAccess, QueryItem},
    system::SystemStage,
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
        let spaces = " ".repeat(self.span.col_no.saturating_sub(1));
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
    pub world: LockedWorldHandle,
    pub scopes: Vec<Scope>,
    pub arena: Arena<InterpreterContext>,
}

impl RuntimeEnv {
    pub fn new(world: LockedWorldHandle, scopes: Vec<Scope>) -> Self {
        Self {
            world,
            scopes,
            arena: Arena::new(),
        }
    }

    pub fn push_scope(&self, inherit: Option<&InterpreterContext>) -> &mut InterpreterContext {
        if let Some(inherit) = inherit {
            self.arena.alloc(InterpreterContext {
                names: inherit.names.clone(),
                allocations: Vec::new(),
                should_break: None,
                should_return: None,
            })
        } else {
            self.arena.alloc(InterpreterContext::new())
        }
    }

    pub fn pop_scope(&self, scope: &InterpreterContext) {
        for allocation in &scope.allocations {
            match allocation {
                ValueHandle::Ref(r) => r.read().despawn(),
                ValueHandle::Mut(m) => m.read().despawn(),
            }
        }
    }
}

#[derive(Clone)]
pub struct InterpreterContext {
    pub names: FxHashMap<String, ValueHandle>,
    pub allocations: Vec<ValueHandle>,
    pub should_break: Option<Option<ValueHandle>>,
    pub should_return: Option<Option<ValueHandle>>,
}

impl InterpreterContext {
    pub fn new() -> Self {
        Self {
            names: FxHashMap::default(),
            allocations: Vec::new(),
            should_break: None,
            should_return: None,
        }
    }

    pub fn alloc_value(&mut self, value: ValueHandle) {
        self.allocations.push(value);
    }

    pub fn interp_system(
        &mut self,
        env: &RuntimeEnv,
        system: &System,
    ) -> anyhow::Result<ValueHandle> {
        self.interp_block(env, &system.block.statements)?;
        Ok(Value::Void.into())
    }

    pub fn interp_block(
        &mut self,
        env: &RuntimeEnv,
        statements: &[Statement],
    ) -> anyhow::Result<()> {
        let scope = env.push_scope(Some(self));
        for statement in statements {
            scope.interp_statement(env, statement)?;
            if let Some(retval) = scope.should_return.take() {
                self.should_return = Some(retval);
                break;
            }
            if let Some(retval) = scope.should_break.take() {
                self.should_break = Some(retval);
                break;
            }
        }
        env.pop_scope(scope);
        Ok(())
    }

    pub fn interp_statement(
        &mut self,
        env: &RuntimeEnv,
        statement: &Statement,
    ) -> anyhow::Result<ValueHandle> {
        match statement {
            Statement::Expr(expr) => {
                self.interp_expr(env, expr)?;
                Ok(Value::Void.into())
            }
            Statement::Break(retval) => {
                let retval = if let Some(retval) = retval {
                    Some(self.interp_expr(env, retval)?)
                } else {
                    None
                };
                self.should_break = Some(retval);
                Ok(Value::Void.into())
            }
            Statement::Return(retval) => {
                let retval = if let Some(retval) = retval {
                    Some(self.interp_expr(env, retval)?)
                } else {
                    None
                };
                self.should_return = Some(retval);
                Ok(Value::Void.into())
            }
            Statement::System(system) => self.interp_system(env, system),
            Statement::Func(_) => todo!(),
            Statement::Component(_) => todo!(),
            Statement::Impl(_) => todo!(),
        }
    }

    pub fn interp_expr(
        &mut self,
        env: &RuntimeEnv,
        expr: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        match &expr.expr {
            Expr::IntLiteral(i) => {
                let value = Value::Int(*i);
                let value = ValueHandle::Mut(SharedLock::new(value));
                self.alloc_value(value.to_owned());
                Ok(value)
            }
            Expr::FloatLiteral(f) => {
                let value = Value::Float(*f);
                let value = ValueHandle::Mut(SharedLock::new(value));
                self.alloc_value(value.to_owned());
                Ok(value)
            }
            Expr::StringLiteral(s) => {
                let value = Value::String(s.clone());
                let value = ValueHandle::Mut(SharedLock::new(value));
                self.alloc_value(value.to_owned());
                Ok(value)
            }
            Expr::Ident(ident) => {
                self.names.get(ident.as_str()).cloned().ok_or_else(|| {
                    runtime_error!(expr.span, format!("Unknown identifier: {}", ident))
                })
            }
            Expr::Infix { op, lhs, rhs } => self.interp_infix(env, op, lhs, rhs),
            Expr::Decl {
                mutability,
                ident,
                initial_value,
            } => self.interp_decl(env, *mutability, ident, initial_value),
            Expr::Call(call) => self.interp_call(env, call),
            Expr::Member { lhs, rhs } => self.interp_member(env, lhs, rhs),
            Expr::Construct { name, args } => self.interp_construct(env, name, args),
            Expr::Query(query) => self.interp_query(env, query),
            Expr::Block(block) => {
                self.interp_block(env, &block.statements)?;
                Ok(Value::Void.into())
            }
            Expr::Loop { condition, block } => self.interp_loop(env, condition.as_deref(), block),
            Expr::Res {
                mutability,
                ident,
                res,
            } => self.interp_res(env, *mutability, ident, res),
            _ => todo!(
                "Implement other expressions:\n{}\n{:?}",
                expr.as_str(),
                expr.expr
            ),
        }
    }

    pub fn interp_res(
        &mut self,
        _env: &RuntimeEnv,
        mutability: bool,
        ident: &SpanExpr,
        res: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        let name = ident.as_str().to_string();

        let res_id = Entity::type_from_name(res.as_str()).ok_or_else(|| {
            runtime_error!(
                res.span,
                format!("Invalid type for resource: {}", res.as_str())
            )
        })?;

        let value = if mutability {
            ValueHandle::Mut(SharedLock::new(Value::Resource(res_id)))
        } else {
            ValueHandle::Ref(SharedLock::new(Value::Resource(res_id)))
        };

        self.names.insert(name, value.to_owned());
        Ok(value)
    }

    pub fn interp_loop(
        &mut self,
        env: &RuntimeEnv,
        condition: Option<&SpanExpr>,
        block: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        match condition {
            Some(condition) => {
                let cond = self.interp_expr(env, condition)?;
                let cond = cond.read();
                if let Value::Bool(false) = &*cond {
                    return Ok(Value::Void.into());
                }
                if let Value::Query(qs) = &*cond {
                    let world = env.world.read();
                    let mut query = world.query();
                    for (_, q) in qs.iter() {
                        match q {
                            QueryBuilderAccess::Entity => query = query.entity(),
                            QueryBuilderAccess::Read(id) => query = query.read_dynamic(*id)?,
                            QueryBuilderAccess::Write(id) => query = query.write_dynamic(*id)?,
                            QueryBuilderAccess::With(id) => query = query.with_dynamic(*id)?,
                            QueryBuilderAccess::Without(id) => {
                                query = query.without_dynamic(*id)?
                            }
                        }
                    }
                    let query = query.build();

                    for results in query.iter() {
                        let scope = env.push_scope(Some(self));

                        for ((name, _), result) in qs.iter().zip(results.into_vec()) {
                            let value = match result {
                                QueryItem::Proxy(p) => ValueHandle::Ref(SharedLock::new(
                                    Value::Data(Data::new_pointer(
                                        p.component.type_uid(),
                                        *p.component.value_uid(),
                                    )),
                                )),
                                QueryItem::ProxyMut(p) => ValueHandle::Mut(SharedLock::new(
                                    Value::Data(Data::new_pointer(
                                        p.component.type_uid(),
                                        *p.component.value_uid(),
                                    )),
                                )),
                                _ => todo!(),
                            };

                            scope.names.insert(name.clone(), value);
                        }

                        scope.interp_expr(env, block)?;

                        if scope.should_break.is_some() {
                            self.should_break = scope.should_break.take();
                        }
                        if scope.should_return.is_some() {
                            self.should_return = scope.should_return.take();
                        }

                        env.pop_scope(scope);
                    }
                } else {
                    loop {
                        self.interp_expr(env, block)?;
                        if self.should_break.is_some() {
                            break;
                        }
                        if self.should_return.is_some() {
                            break;
                        }

                        let condition = self.interp_expr(env, condition)?;
                        let condition = condition.read();
                        if let Value::Bool(false) = &*condition {
                            break;
                        }
                    }
                }
            }
            None => loop {
                self.interp_expr(env, block)?;
                if self.should_break.is_some() {
                    break;
                }
                if self.should_return.is_some() {
                    break;
                }
            },
        }
        Ok(Value::Void.into())
    }

    pub fn interp_query(&mut self, env: &RuntimeEnv, query: &Query) -> anyhow::Result<ValueHandle> {
        let names = query
            .components
            .iter()
            .map(|c| c.name.as_str().to_owned())
            .collect::<Vec<_>>();
        let query = query.build_on_world(env.world.clone())?;

        #[allow(clippy::map_identity)]
        let query = names
            .into_iter()
            .zip(query)
            .map(|(n, q)| (n, q))
            .collect::<Vec<_>>();
        Ok(Value::Query(query).into())
    }

    pub fn interp_construct(
        &mut self,
        env: &RuntimeEnv,
        name: &SpanExpr,
        args: &[(String, SpanExpr)],
    ) -> anyhow::Result<ValueHandle> {
        let name = name.as_str();
        let ty = Entity::allocate_type(Some(name));
        let e = env.world.write().create_entity()?;
        for (arg_name, arg) in args {
            let value = self.interp_expr(env, arg)?;
            let value_uid = value.read().value_uid(env).map_err(|_| {
                runtime_error!(
                    arg.span,
                    format!("Invalid argument for constructing {}: {}", name, arg_name)
                )
            })?;
            let mut world = env.world.write();
            let value_ty = value_uid.type_id().unwrap();

            todo!("Implement constructing entities")
        }

        Ok(Value::Data(Data::new_pointer(ty, e)).into())
    }

    pub fn interp_call(&mut self, env: &RuntimeEnv, call: &Call) -> anyhow::Result<ValueHandle> {
        match call.name.as_str() {
            "print" => {
                for arg in &call.args {
                    let value = self.interp_expr(env, arg)?;
                    let value = value.display(env);
                    print!("{}", value);
                }
                println!();
                Ok(Value::Void.into())
            }
            #[cfg(feature = "glam")]
            "vec3" => {
                let x_val = self.interp_expr(env, &call.args[0])?;
                let y_val = self.interp_expr(env, &call.args[1])?;
                let z_val = self.interp_expr(env, &call.args[2])?;
                let x = x_val.read().as_float().ok_or_else(|| {
                    runtime_error!(call.name.span, "Invalid argument for vec3: x")
                })?;
                x_val.read().despawn();
                let y = y_val.read().as_float().ok_or_else(|| {
                    runtime_error!(call.name.span, "Invalid argument for vec3: y")
                })?;
                y_val.read().despawn();
                let z = z_val.read().as_float().ok_or_else(|| {
                    runtime_error!(call.name.span, "Invalid argument for vec3: z")
                })?;
                z_val.read().despawn();

                let v = glam::Vec3::new(x, y, z);
                let data = Data::new_dynamic(v);
                let data = ValueHandle::Mut(SharedLock::new(Value::Data(data)));
                self.alloc_value(data.to_owned());
                Ok(data)
            }
            #[cfg(feature = "glam")]
            "quat" => {
                let axis = self.interp_expr(env, &call.args[0])?;
                let angle_val = self.interp_expr(env, &call.args[1])?;

                let axis = axis.read();
                let axis_data = axis.as_data().ok_or_else(|| {
                    runtime_error!(call.name.span, "Invalid argument for quat: axis")
                })?;
                let axis = axis_data.as_ref::<glam::Vec3>().ok_or_else(|| {
                    runtime_error!(call.args[0].span, "Invalid argument for quat: axis")
                })?;

                let angle = angle_val.read().as_float().ok_or_else(|| {
                    runtime_error!(call.args[1].span, "Invalid argument for quat: angle")
                })?;
                angle_val.read().despawn();

                let q = glam::Quat::from_axis_angle(*axis, angle);
                let data = Data::new_dynamic(q);
                let data = ValueHandle::Mut(SharedLock::new(Value::Data(data)));
                self.alloc_value(data.to_owned());
                Ok(data)
            }
            _ => todo!("Implement other calls: {:?}", call),
        }
    }

    pub fn interp_member(
        &mut self,
        env: &RuntimeEnv,
        lhs: &SpanExpr,
        rhs: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        let lhs_val = self.interp_expr(env, lhs)?;

        match &rhs.expr {
            Expr::Ident(_) => bail!(rhs.span, "Invalid member access: accessing member fields of struct components is no longer supported; use a method instead"),
            Expr::Call(call) => {
                let world = env.world.read();
                let mut args = Vec::new();
                for arg in &call.args {
                    let span = arg.span.clone();
                    let arg = self.interp_expr(env, arg)?;
                    let arg_lock = arg.read();
                    if let Value::Data(data) = &*arg_lock {
                        args.push(crate::component::MethodArg::Owned(data.to_owned()));
                    } else {
                        let arg = arg_lock.to_owned_data().map_err(|_| {
                            runtime_error!(span, "Invalid argument for method call")
                        })?;
                        args.push(crate::component::MethodArg::Owned(arg));
                    }
                }
                match lhs_val {
                    ValueHandle::Ref(ref lhs_data) => {
                        let lhs_data = lhs_data.read();
                        let data = match &*lhs_data {
                            Value::Resource(res) => {
                                let res = world.get_resource(*res).ok_or_else(|| {
                                    runtime_error!(
                                        lhs.span,
                                        format!("Invalid resource: {}", lhs_val.display(env))
                                    )
                                })?;
                                res.to_owned().unwrap()
                            }
                            Value::Data(data) => data.to_owned(),
                            _ => todo!(),
                        };

                        let vtable = match &data {
                            Data::Dynamic(d) => {
                                d.data().script_vtable()
                            },
                            Data::Pointer(p) => {
                                // deref one level
                                let data_ref = world.get(p.target_value_uid(), p.target_type_uid()).unwrap();
                                let vtable = if let Some(data_ref) = data_ref.as_dynamic() {
                                    data_ref.data().data().script_vtable()
                                } else {
                                    bail!(rhs.span, "Invalid method call: No such method")
                                };

                                vtable
                            }
                        };

                        if let Some(method) = vtable.get_method(call.name.as_str()) {
                            match method.takes_self {
                                TakesSelf::None => {},
                                TakesSelf::RefMut => bail!(call.name.span, "Invalid method call: Method takes self by mutable reference"),
                                TakesSelf::Ref => {
                                    match &data {
                                        Data::Dynamic(d) => {
                                            let d = world.get(d.value_uid(), d.type_uid()).unwrap();
                                            args.insert(0, MethodArg::Ref(d));
                                        }
                                        Data::Pointer(p) => {
                                            let d = world.storage().find(p.target_type_uid(), p.target_value_uid()).unwrap();
                                            args.insert(0, MethodArg::Ref(d));
                                        }
                                    }
                                },
                            }
                        } else {
                            bail!(call.name.span, "Invalid method call: No such method")
                        }


                        let mut result = vtable.call_method(call.name.as_str(), args)?;

                        ensure!(
                            result.len() == 1,
                            runtime_error!(
                                call.name.span,
                                format!(
                                    "Invalid return value for method call: {}",
                                    call.name.as_str()
                                )
                            )
                        );
                        let result = result.pop().unwrap();

                        Ok(ValueHandle::Mut(SharedLock::new(Value::Data(result))))
                    }
                    ValueHandle::Mut(ref lhs_data) => {
                        let lhs_data = lhs_data.write();
                        let data = match &*lhs_data {
                            Value::Resource(res) => {
                                let res = world.get_resource(*res).ok_or_else(|| {
                                    runtime_error!(
                                        lhs.span,
                                        format!("Invalid resource: {}", lhs_val.display(env))
                                    )
                                })?;
                                res.to_owned().unwrap()
                            }
                            Value::Data(data) => data.to_owned(),
                            _ => todo!(),
                        };

                        let vtable = match &data {
                            Data::Dynamic(d) => {
                                d.data().script_vtable()
                            },
                            Data::Pointer(p) => {
                                // deref one level
                                let data_ref = world.storage().find(p.target_type_uid(), p.target_value_uid()).unwrap();
                                let vtable = if let Some(data_ref) = data_ref.as_dynamic() {
                                    data_ref.data().data().script_vtable()
                                } else {
                                    bail!(rhs.span, "Invalid method call: No such method")
                                };

                                vtable
                            }
                        };

                        if let Some(method) = vtable.get_method(call.name.as_str()) {
                            match method.takes_self {
                                TakesSelf::None => {},
                                TakesSelf::RefMut => {
                                    match &data {
                                        Data::Dynamic(d) => {
                                            let d = world.storage().find_mut(d.type_uid(), d.value_uid()).unwrap();
                                            args.insert(0, MethodArg::Mut(d));
                                        }
                                        Data::Pointer(p) => {
                                            let d = world.storage().find_mut(p.target_type_uid(), p.target_value_uid()).unwrap();
                                            args.insert(0, MethodArg::Mut(d));
                                        }
                                    }
                                }
                                TakesSelf::Ref => {
                                    match &data {
                                        Data::Dynamic(d) => {
                                            let d = world.storage().find(d.type_uid(), d.value_uid()).unwrap();
                                            args.insert(0, MethodArg::Ref(d));
                                        }
                                        Data::Pointer(p) => {
                                            let d = world.storage().find(p.target_type_uid(), p.target_value_uid()).unwrap();
                                            args.insert(0, MethodArg::Ref(d));
                                        }
                                    }
                                },
                            }
                        } else {
                            bail!(call.name.span, "Invalid method call: No such method")
                        }


                        let mut result = vtable.call_method(call.name.as_str(), args)?;

                        ensure!(
                            result.len() == 1,
                            runtime_error!(
                                call.name.span,
                                format!(
                                    "Invalid return value for method call: {}",
                                    call.name.as_str()
                                )
                            )
                        );
                        let result = result.pop().unwrap();

                        Ok(ValueHandle::Mut(SharedLock::new(Value::Data(result))))
                    }
                }
            }
            _ => bail!(rhs.span, "Invalid member access"),
        }
    }

    pub fn interp_infix(
        &mut self,
        env: &RuntimeEnv,
        op: &str,
        lhs: &SpanExpr,
        rhs: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        let lhs_val = self.interp_expr(env, lhs)?;
        let rhs_val = self.interp_expr(env, rhs)?;
        let out = lhs_val.infix(op, &rhs_val, env)?;
        let out = ValueHandle::Mut(SharedLock::new(out));
        if matches!(&*out.read(), Value::Data(_)) {
            self.alloc_value(out.to_owned());
        }
        Ok(out)
    }

    pub fn interp_decl(
        &mut self,
        env: &RuntimeEnv,
        mutability: bool,
        ident: &SpanExpr,
        initial_value: &SpanExpr,
    ) -> anyhow::Result<ValueHandle> {
        let value = self.interp_expr(env, initial_value)?;
        let name = ident.as_str().to_string();
        let value = if mutability {
            match value {
                ValueHandle::Ref(r) => ValueHandle::Mut(r.to_owned()),
                ValueHandle::Mut(m) => ValueHandle::Mut(m.to_owned()),
            }
        } else {
            match value {
                ValueHandle::Ref(r) => ValueHandle::Ref(r.to_owned()),
                ValueHandle::Mut(m) => ValueHandle::Ref(m.to_owned()),
            }
        };
        self.names.insert(name, value.to_owned());
        Ok(value)
    }
}

impl Default for InterpreterContext {
    fn default() -> Self {
        Self::new()
    }
}

pub trait BuildOnWorld {
    type Output;

    fn build_on_world(&self, world: LockedWorldHandle) -> anyhow::Result<Self::Output>;
}

impl BuildOnWorld for Script {
    type Output = ();

    fn build_on_world(&self, world: LockedWorldHandle) -> anyhow::Result<()> {
        for scope in &self.scopes {
            match scope {
                Scope::System(ref system) => {
                    let system_clone = system.clone();
                    let scopes = self.scopes.clone();
                    let run_fn = move |world| {
                        let env = RuntimeEnv::new(world, scopes.clone());
                        let ctx = env.push_scope(None);
                        ctx.interp_system(&env, &system_clone).unwrap();
                        env.pop_scope(ctx);
                    };

                    let tag = system.tag.clone();

                    let stage = match tag.as_deref() {
                        Some("@startup") => SystemStage::Startup,
                        Some("@pre_update") => SystemStage::PreUpdate,
                        Some("@update") => SystemStage::Update,
                        Some("@post_update") => SystemStage::PostUpdate,
                        Some("@shutdown") => SystemStage::Shutdown,
                        Some(tag) => todo!("Unknown system tag: {}", tag),
                        _ => SystemStage::Update,
                    };

                    world.write().add_system(stage, run_fn);
                }

                Scope::Component(_component) => {
                    todo!("Implement component registration")
                }
                Scope::Func(_) => {}
                Scope::Impl(_) => {}
                Scope::Program(_) => unreachable!(),
            }
        }

        Ok(())
    }
}

impl BuildOnWorld for Query {
    type Output = Vec<QueryBuilderAccess>;

    fn build_on_world(&self, world: LockedWorldHandle) -> anyhow::Result<Vec<QueryBuilderAccess>> {
        let mut query = Vec::new();
        for component in &self.components {
            let id = component.build_on_world(world.clone())?;
            if component.mutability {
                query.push(QueryBuilderAccess::Write(id));
            } else {
                query.push(QueryBuilderAccess::Read(id));
            }
        }
        for with in &self.with {
            let id = with.as_str().to_string().build_on_world(world.clone())?;
            query.push(QueryBuilderAccess::With(id));
        }
        for without in &self.without {
            let id = without.as_str().to_string().build_on_world(world.clone())?;
            query.push(QueryBuilderAccess::Without(id));
        }
        Ok(query)
    }
}

impl BuildOnWorld for TypedIdent {
    type Output = Entity;

    fn build_on_world(&self, _world: LockedWorldHandle) -> anyhow::Result<Entity> {
        Ok(Entity::allocate_type(Some(self.ty.as_str())))
    }
}

impl BuildOnWorld for String {
    type Output = Entity;

    fn build_on_world(&self, _world: LockedWorldHandle) -> anyhow::Result<Entity> {
        Ok(Entity::allocate_type(Some(self.as_str())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn test_interp_literals() {
        let world_handle = World::new_handle();
        let env = RuntimeEnv::new(world_handle.clone(), Vec::new());
        let ctx = env.push_scope(None);
        {
            let value = ctx
                .interp_expr(
                    &env,
                    &SpanExpr {
                        span: Span::default(),
                        expr: Expr::IntLiteral(42),
                    },
                )
                .unwrap()
                .deep_copy();
            assert_eq!(value.as_int(), Some(42));
        }

        {
            let value = ctx
                .interp_expr(
                    &env,
                    &SpanExpr {
                        span: Span::default(),
                        expr: Expr::FloatLiteral(4.20),
                    },
                )
                .unwrap()
                .deep_copy();
            assert_eq!(value.as_float(), Some(4.20));
        }
    }

    #[test]
    fn test_interp_infix() {
        let world_handle = World::new_handle();
        let env = RuntimeEnv::new(world_handle.clone(), Vec::new());
        let ctx = env.push_scope(None);
        {
            let value = ctx
                .interp_expr(
                    &env,
                    &SpanExpr {
                        span: Span::default(),
                        expr: Expr::Infix {
                            op: "+".to_string(),
                            lhs: Box::new(SpanExpr {
                                span: Span::default(),
                                expr: Expr::IntLiteral(40),
                            }),
                            rhs: Box::new(SpanExpr {
                                span: Span::default(),
                                expr: Expr::IntLiteral(2),
                            }),
                        },
                    },
                )
                .unwrap()
                .deep_copy();
            assert_eq!(value.as_int(), Some(42));
        }

        {
            let value = ctx
                .interp_expr(
                    &env,
                    &SpanExpr {
                        span: Span::default(),
                        expr: Expr::Infix {
                            op: "+".to_string(),
                            lhs: Box::new(SpanExpr {
                                span: Span::default(),
                                expr: Expr::FloatLiteral(4.0),
                            }),
                            rhs: Box::new(SpanExpr {
                                span: Span::default(),
                                expr: Expr::FloatLiteral(0.2),
                            }),
                        },
                    },
                )
                .unwrap()
                .deep_copy();
            assert_eq!(value.as_float(), Some(4.2));
        }
    }

    #[test]
    fn test_interp_infix_assign() {
        let world_handle = World::new_handle();
        let env = RuntimeEnv::new(world_handle.clone(), Vec::new());
        let ctx = env.push_scope(None);
        {
            let value = ctx
                .interp_decl(
                    &env,
                    true,
                    &SpanExpr {
                        span: Span {
                            line_no: 1,
                            col_no: 1,
                            line: "".to_string(),
                            fragment: "x".to_string(),
                        },
                        expr: Expr::Ident("x".to_string()),
                    },
                    &SpanExpr {
                        span: Span::default(),
                        expr: Expr::IntLiteral(42),
                    },
                )
                .unwrap()
                .deep_copy();
            assert_eq!(value.as_int(), Some(42));

            let value = ctx
                .interp_expr(
                    &env,
                    &SpanExpr {
                        span: Span::default(),
                        expr: Expr::Infix {
                            op: "+=".to_string(),
                            lhs: Box::new(SpanExpr {
                                span: Span::default(),
                                expr: Expr::Ident("x".to_string()),
                            }),
                            rhs: Box::new(SpanExpr {
                                span: Span::default(),
                                expr: Expr::IntLiteral(2),
                            }),
                        },
                    },
                )
                .unwrap()
                .deep_copy();
            assert_eq!(value.as_int(), Some(44));
        }
    }

    #[test]
    fn test_interp_decl() {
        let world_handle = World::new_handle();
        let env = RuntimeEnv::new(world_handle.clone(), Vec::new());
        let ctx = env.push_scope(None);
        {
            let value = ctx
                .interp_decl(
                    &env,
                    true,
                    &SpanExpr {
                        span: Span::default(),
                        expr: Expr::Ident("x".to_string()),
                    },
                    &SpanExpr {
                        span: Span::default(),
                        expr: Expr::IntLiteral(42),
                    },
                )
                .unwrap()
                .deep_copy();
            assert_eq!(value.as_int(), Some(42));
        }

        {
            let value = ctx
                .interp_decl(
                    &env,
                    false,
                    &SpanExpr {
                        span: Span::default(),
                        expr: Expr::Ident("y".to_string()),
                    },
                    &SpanExpr {
                        span: Span::default(),
                        expr: Expr::FloatLiteral(4.20),
                    },
                )
                .unwrap()
                .deep_copy();
            assert_eq!(value.as_float(), Some(4.20));
        }
    }
}
